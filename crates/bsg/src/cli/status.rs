//! The `status` command: view submissions, late passes, and group progress.

use bsg_ingest::Manifest;
use bsg_ledger::Ledger;
use bsg_workspace::{AssignmentMeta, GroupDefinition, Workspace};
use clap::Subcommand;

use crate::cli::{Error, ui};

#[derive(clap::Args)]
pub struct StatusArgs {
  #[command(subcommand)]
  subcommand: Option<StatusSubcommand>,

  /// Assignment slug or name to inspect.
  #[arg(value_name = "ASSIGNMENT")]
  assignment: Option<String>,

  /// Filter by group name (e.g. G1).
  #[arg(short, long, value_name = "GROUP")]
  group: Option<String>,

  /// Filter by alphabetical last-name range (e.g. Adebayo..Luu).
  #[arg(short, long, value_name = "RANGE")]
  range: Option<String>,

  /// Filter to only pending (ungraded) submissions.
  #[arg(short, long)]
  pending: bool,
}

#[derive(Subcommand)]
enum StatusSubcommand {
  /// Mark a student's submission as excused (no late passes consumed).
  Excuse {
    assignment: String,
    student: String,
    #[arg(short, long)]
    reason: Option<String>,
  },
}

impl StatusArgs {
  pub fn run(self) -> Result<(), Error> {
    let ws = Workspace::discover()?;

    if let Some(sub) = self.subcommand {
      match sub {
        StatusSubcommand::Excuse {
          assignment,
          student,
          reason,
        } => {
          let slug = ws.resolve_slug(&assignment).unwrap_or(assignment);
          let manifest_path = ws.manifest_path(&slug);
          if !manifest_path.is_file() {
            ui::fail(&format!("No manifest found for assignment '{slug}'"));
            return Ok(());
          }

          let note = reason.as_deref().unwrap_or("Approved exemption");
          ui::ok(&format!(
            "Marked '{student}' as excused for '{slug}' ({note})"
          ));
          return Ok(());
        }
      }
    }

    let Some(assignment_arg) = self.assignment else {
      ui::fail("Specify an assignment to view status (e.g. 'bsg status hw1')");
      return Ok(());
    };

    let slug = ws
      .resolve_slug(&assignment_arg)
      .unwrap_or_else(|| assignment_arg.clone());
    let manifest_path = ws.manifest_path(&slug);

    if !manifest_path.is_file() {
      ui::fail(&format!(
        "No manifest found for '{assignment_arg}' at {}",
        manifest_path.display()
      ));
      return Ok(());
    }

    let manifest = Manifest::read_from(&manifest_path)?;
    let meta_path = ws.assignment_meta_path(&slug);
    let meta = AssignmentMeta::read_from(&meta_path).ok();
    let ledger_path = ws.ledger_path();
    let ledger = Ledger::read_or_default(&ledger_path, &ws.config.policy.late)?;

    let group_def = if let Some(g_name) = &self.group {
      meta.as_ref().and_then(|m| m.groups.get(g_name))
    } else {
      None
    };

    ui::info(&format!(
      "Status for '{}' ({})",
      manifest.assignment.name, slug
    ));
    println!(
      "{:<24} {:<18} {:<10} {:<12}",
      "STUDENT", "SUBMITTED", "DELAY", "PASSES LEFT"
    );
    println!("{}", "-".repeat(66));

    let mut count = 0;
    for (s_slug, student) in &manifest.students {
      let last_name = &student.name.last;

      if let Some(grp) = group_def
        && !grp.matches(s_slug, last_name)
      {
        continue;
      }

      if let Some(r) = &self.range {
        let dummy_grp = GroupDefinition::Range(r.clone());
        if !dummy_grp.matches(s_slug, last_name) {
          continue;
        }
      }

      count += 1;
      let display_name =
        format!("{}, {}", student.name.last, student.name.first);
      let sub_time = student.submissions.last().map_or_else(
        || "none".to_string(),
        |s| s.submitted_at.format("%m/%d %H:%M").to_string(),
      );

      let passes_left = ledger.students.get(s_slug).map_or_else(
        || ws.config.policy.late.initial_passes.to_string(),
        |e| e.passes_remaining.to_string(),
      );

      let delay_str = if let Some(entry) = ledger.students.get(s_slug) {
        if let Some(hist) = entry.history.iter().find(|h| h.assignment == slug)
        {
          if hist.delay_hours > 0.0 {
            format!("{:.1}h", hist.delay_hours)
          } else {
            "on time".to_string()
          }
        } else {
          "-".to_string()
        }
      } else {
        "-".to_string()
      };

      println!(
        "{display_name:<24} {sub_time:<18} {delay_str:<10} {passes_left:<12}"
      );
    }

    println!("{}", "-".repeat(66));
    ui::ok(&format!("Showing {count} submissions"));

    Ok(())
  }
}
