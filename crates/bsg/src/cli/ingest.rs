//! The `ingest` command: discovers Brightspace downloads, selects files, and
//! writes manifests.

use std::fs::{self, File};
use std::io;
use std::path::PathBuf;

use bsg_ingest as ingest;
use bsg_ingest::{DestinationState, Manifest};
use bsg_ledger::Ledger;
use bsg_workspace::{AssignmentMeta, Workspace};
use zip::ZipArchive;

use crate::cli::{Error, ui};

/// The manifest file name generated for ingested assignments.
const MANIFEST_FILENAME: &str = ".manifest.json";

#[derive(clap::Args)]
pub struct IngestArgs {
  /// Brightspace downloads, or directories whose downloads to ingest.
  /// Defaults to workspace inputs path if omitted.
  #[arg(value_name = "TARGET")]
  targets: Vec<PathBuf>,

  /// Directory to extract into instead of beside each download.
  #[arg(short = 'C', long = "directory", value_name = "DIR")]
  output_dir: Option<PathBuf>,

  /// Overwrite non-empty output directories.
  #[arg(short, long)]
  force: bool,
}

struct ValidatedDownload {
  zip: ZipArchive<File>,
  manifest: Manifest,
  tasks: Vec<ingest::Task>,
  output_dir: PathBuf,
  assignment_name: String,
}

impl IngestArgs {
  pub fn run(self) -> Result<(), Error> {
    let ws = Workspace::discover().ok();

    let targets = if self.targets.is_empty() {
      if let Some(w) = &ws {
        vec![w.root.join(&w.config.paths.inputs)]
      } else {
        return Err(Error::Io(io::Error::new(
          io::ErrorKind::InvalidInput,
          "No targets provided and no .bsg workspace discovered",
        )));
      }
    } else {
      self.targets
    };

    // Discover and resolve all unique assignment archives across targets.
    let assignments = ingest::find(&targets)?;

    // Diagnose input archives and warn on ambiguous duplicates.
    let diagnosis = ingest::check_src(assignments);
    for group in &diagnosis.duplicates {
      ui::warn(&format!(
        "Multiple archives found for '{}'; skipping ambiguous input",
        group[0].name
      ));
    }

    // Validate and plan every download before writing anything to disk.
    let mut validated_downloads = Vec::new();
    for assignment in diagnosis.ready {
      let output_dir = match &self.output_dir {
        Some(dir) => dir.join(&assignment.name),
        None => {
          if let Some(w) = &ws {
            w.root
              .join(&w.config.paths.assignments)
              .join(&assignment.name)
          } else {
            assignment.path.with_file_name(&assignment.name)
          }
        }
      };

      let state =
        ingest::destination(&assignment, &output_dir, MANIFEST_FILENAME)?;
      match state {
        DestinationState::UpToDate { downloaded } => {
          ui::info(&format!(
            "'{}' is up to date (downloaded {downloaded})",
            assignment.name
          ));
        }
        DestinationState::DateMismatch { existing, incoming } => {
          ui::warn(&format!(
            "'{}' exists with different download date ({existing} vs incoming \
             {incoming})",
            assignment.name
          ));
        }
        DestinationState::NonEmptyWithoutManifest => {
          ui::warn(&format!(
            "'{}' destination exists and is not empty without a manifest",
            assignment.name
          ));
        }
        DestinationState::Fresh | _ => {}
      }

      if !matches!(state, DestinationState::Fresh) {
        if !self.force {
          continue;
        }
        fs::remove_dir_all(&output_dir)?;
      }

      let file = File::open(&assignment.path)?;
      let mut zip = ZipArchive::new(file)?;
      let students = ingest::scan(&mut zip)?;
      let assignment_name = assignment.name.clone();
      let (manifest, tasks) = ingest::select(assignment, &students);
      validated_downloads.push(ValidatedDownload {
        zip,
        manifest,
        tasks,
        output_dir,
        assignment_name,
      });
    }

    // Execute extractions and write manifests.
    for mut download in validated_downloads {
      let mut spinner = ui::Spinner::new();
      spinner.step(&format!("Extracting '{}'...", download.assignment_name));

      ingest::extract(
        &mut download.zip,
        &download.tasks,
        &download.output_dir,
      )?;

      // Write manifest beside files
      let manifest_path = download.output_dir.join(MANIFEST_FILENAME);
      download.manifest.write_to(&manifest_path)?;

      // If inside a workspace, also write to
      // .bsg/assignments/<slug>/manifest.json
      if let Some(w) = &ws {
        let slug = w
          .resolve_slug(&download.assignment_name)
          .unwrap_or_else(|| slugify(&download.assignment_name));

        let ws_manifest_path = w.manifest_path(&slug);
        let _ = download.manifest.write_to(&ws_manifest_path);

        // Update ledger if assignment has due date
        let meta_path = w.assignment_meta_path(&slug);
        if let Ok(meta) = AssignmentMeta::read_from(&meta_path)
          && let Some(due) = meta.due
        {
          let ledger_path = w.ledger_path();
          if let Ok(mut ledger) =
            Ledger::read_or_default(&ledger_path, &w.config.policy.late)
          {
            for (s_slug, student) in &download.manifest.students {
              if let Some(latest_sub) = student.submissions.last() {
                ledger.record_submission(
                  s_slug,
                  &slug,
                  latest_sub.submitted_at,
                  due,
                  false.into(),
                  None,
                );
              }
            }
            let _ = ledger.write_to(&ledger_path);
          }
        }
      }

      spinner.done(&format!(
        "Ingested '{}' ({} submissions)",
        download.assignment_name,
        download.manifest.students.len()
      ));
    }

    Ok(())
  }
}

/// Helper to convert a human-readable title into a slug if unknown.
fn slugify(title: &str) -> String {
  title
    .to_lowercase()
    .chars()
    .map(|c| if c.is_alphanumeric() { c } else { '_' })
    .collect::<String>()
    .split('_')
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("_")
}
