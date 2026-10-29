//! The `grade` command: unified grading, test execution, and notebook
//! preparation.

use std::fs;
use std::path::{Path, PathBuf};

use bsg_ingest::Manifest;
use bsg_notebook::Notebook;
use bsg_runner as runner;
use bsg_workspace::{AssignmentMeta, GroupDefinition, Workspace};

use crate::cli::{Error, ui};

#[derive(clap::Args)]
pub struct GradeArgs {
  /// Assignment slug or name to grade.
  #[arg(value_name = "ASSIGNMENT")]
  assignment: String,

  /// Specific student slug to grade.
  #[arg(value_name = "STUDENT")]
  student: Option<String>,

  /// Filter by group name (e.g. G1).
  #[arg(short, long, value_name = "GROUP")]
  group: Option<String>,

  /// Filter by alphabetical last-name range (e.g. Adebayo..Luu).
  #[arg(short, long, value_name = "RANGE")]
  range: Option<String>,

  /// Only run automated test scripts without full grading.
  #[arg(long)]
  check_only: bool,

  /// Only sanitize and format notebook without grading.
  #[arg(long)]
  prep_only: bool,
}

impl GradeArgs {
  pub fn run(self) -> Result<(), Error> {
    let ws = Workspace::discover()?;
    let slug = ws
      .resolve_slug(&self.assignment)
      .unwrap_or_else(|| self.assignment.clone());

    let manifest_path = ws.manifest_path(&slug);
    if !manifest_path.is_file() {
      ui::fail(&format!("No manifest found for assignment '{slug}'"));
      return Ok(());
    }

    let manifest = Manifest::read_from(&manifest_path)?;
    let meta_path = ws.assignment_meta_path(&slug);
    let meta = AssignmentMeta::read_from(&meta_path).ok();
    let group_def = if let Some(g_name) = &self.group {
      meta.as_ref().and_then(|m| m.groups.get(g_name))
    } else {
      None
    };

    let starter_nb = ws
      .starter_path(&slug)
      .is_file()
      .then(|| Notebook::read_from(&ws.starter_path(&slug)).ok())
      .flatten();
    let starter_prompts =
      starter_nb.as_ref().map(Notebook::extract_starter_prompts);
    let tests_dir = ws.tests_dir(&slug);
    let test_scripts = runner::find_test_scripts(&tests_dir);

    let assignment_folder = ws
      .root
      .join(&ws.config.paths.assignments)
      .join(&manifest.assignment.name);

    let mut students_to_grade = Vec::new();
    for (s_slug, student) in &manifest.students {
      if let Some(target_student) = &self.student
        && !s_slug.eq_ignore_ascii_case(target_student)
      {
        continue;
      }

      if let Some(grp) = group_def
        && !grp.matches(s_slug, &student.name.last)
      {
        continue;
      }

      if let Some(r) = &self.range {
        let dummy = GroupDefinition::Range(r.clone());
        if !dummy.matches(s_slug, &student.name.last) {
          continue;
        }
      }

      students_to_grade.push((s_slug.clone(), student.name.clone()));
    }

    if students_to_grade.is_empty() {
      ui::warn("No matching students found to grade");
      return Ok(());
    }

    ui::info(&format!(
      "Grading {} student(s) for '{}'",
      students_to_grade.len(),
      manifest.assignment.name
    ));

    for (s_slug, name) in &students_to_grade {
      let candidate_ipynb = assignment_folder.join(format!("{s_slug}.ipynb"));
      let candidate_dir = assignment_folder.join(s_slug);

      let target_file = if candidate_ipynb.is_file() {
        candidate_ipynb
      } else if candidate_dir.is_dir() {
        find_primary_file_in_dir(&candidate_dir).unwrap_or(candidate_dir)
      } else {
        assignment_folder.join(s_slug)
      };

      if !target_file.exists() {
        ui::warn(&format!(
          "Submission file missing for {}, {}",
          name.last, name.first
        ));
        continue;
      }

      println!(
        "\n{}--- Student: {} {} ({}) ---{}",
        ui::COLOR_CYAN,
        name.first,
        name.last,
        s_slug,
        ui::COLOR_RESET
      );

      // Run tests if present
      if !test_scripts.is_empty() {
        for script in &test_scripts {
          let script_name =
            script.file_name().unwrap_or_default().to_string_lossy();
          match runner::run_test_script(script, &target_file, None) {
            Ok(Some(res)) => {
              if res.passed {
                ui::ok(&format!(
                  "{script_name}: {} ({:.1}/{:.1})",
                  res.task, res.score, res.max_score
                ));
              } else {
                ui::fail(&format!(
                  "{script_name}: {} - {}",
                  res.task, res.reasoning
                ));
              }
            }
            Ok(None) => ui::warn(&format!("{script_name}: No result")),
            Err(e) => ui::fail(&format!("{script_name} execution error: {e}")),
          }
        }
      }

      if self.check_only {
        continue;
      }

      // Notebook prep / sanitization
      if (target_file.extension().is_some_and(|ext| ext == "ipynb")
        || is_json_notebook(&target_file))
        && let Ok(nb) = Notebook::read_from(&target_file)
      {
        let sanitized = nb.sanitize_to_markdown(starter_prompts.as_ref());
        if self.prep_only {
          let out_file = target_file.with_extension("grade.md");
          let _ = fs::write(&out_file, sanitized);
          ui::ok(&format!("Saved clean markdown to {}", out_file.display()));
        }
      }
    }

    Ok(())
  }
}

fn find_primary_file_in_dir(dir: &Path) -> Option<PathBuf> {
  let entries = fs::read_dir(dir).ok()?;
  let mut candidates = Vec::new();
  for entry in entries.flatten() {
    let p = entry.path();
    if p.is_file() {
      let name = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
      if name.contains("final") {
        return Some(p);
      }
      if p.extension().is_some_and(|ext| ext == "ipynb") {
        candidates.push(p);
      }
    }
  }
  candidates.into_iter().next()
}

fn is_json_notebook(p: &Path) -> bool {
  if let Ok(content) = fs::read_to_string(p) {
    content.contains("\"cells\"") && content.contains("\"nbformat\"")
  } else {
    false
  }
}
