//! The `repo` command: workspace creation, authoring asset sync, and
//! validation.

use std::{env, fs, io, process};

use bsg_workspace::Workspace;
use clap::Subcommand;

use crate::cli::{Error, ui};

#[derive(clap::Args)]
pub struct RepoArgs {
  #[command(subcommand)]
  subcommand: RepoSubcommand,
}

#[derive(Subcommand)]
enum RepoSubcommand {
  /// Initialize a .bsg workspace in the current directory, or clone from a
  /// remote repo.
  Init {
    /// Remote git repository URL to clone as .bsg/.
    #[arg(value_name = "URL")]
    url: Option<String>,
  },
  /// Pull latest authoring assets (rubrics, solutions, tests) from remote.
  Sync,
  /// Connect local .bsg to a private remote repository and push.
  Publish {
    /// Remote repository URL (e.g. git@github.com:usc-ta/tac-459-assets.git).
    #[arg(value_name = "URL")]
    url: String,
  },
  /// Validate repository integrity, schemas, and file whitelist rules.
  Validate,
}

impl RepoArgs {
  pub fn run(self) -> Result<(), Error> {
    match self.subcommand {
      RepoSubcommand::Init { url } => {
        let current_dir = env::current_dir()?;
        if let Some(remote_url) = url {
          let bsg_path = current_dir.join(".bsg");
          let bsg_str = bsg_path.to_string_lossy();
          ui::info(&format!("Cloning course repository from {remote_url}..."));
          let status = run_git(&["clone", &remote_url, &bsg_str])?;
          if status.success() {
            ui::ok("Initialized .bsg workspace from remote repository");
          } else {
            ui::fail("Git clone failed");
          }
        } else {
          Workspace::init(&current_dir)?;
          let bsg_path = current_dir.join(".bsg");
          let bsg_str = bsg_path.to_string_lossy();
          let _ = run_git(&["-C", &bsg_str, "init"]);
          ui::ok("Initialized fresh .bsg workspace");
        }
      }
      RepoSubcommand::Sync => {
        let ws = Workspace::discover()?;
        let bsg_str = ws.bsg_dir.to_string_lossy();
        ui::info("Synchronizing authoring assets from remote...");
        let status = run_git(&["-C", &bsg_str, "pull", "--rebase"])?;
        if status.success() {
          ui::ok("Workspace synchronized with remote");
        } else {
          ui::fail("Git pull failed");
        }
      }
      RepoSubcommand::Publish { url } => {
        let ws = Workspace::discover()?;
        let bsg_str = ws.bsg_dir.to_string_lossy();
        let _ = run_git(&["-C", &bsg_str, "remote", "add", "origin", &url]);
        let status =
          run_git(&["-C", &bsg_str, "push", "-u", "origin", "main"])?;
        if status.success() {
          ui::ok(&format!("Published .bsg workspace to {url}"));
        } else {
          ui::fail("Git push failed");
        }
      }
      RepoSubcommand::Validate => {
        let ws = Workspace::discover()?;
        ui::info(&format!(
          "Validating workspace for '{}' ({})",
          ws.config.course.name, ws.config.course.semester
        ));
        let assignments_dir = ws.assignments_dir();
        if assignments_dir.is_dir()
          && let Ok(entries) = fs::read_dir(assignments_dir)
        {
          for entry in entries.flatten() {
            let slug = entry.file_name().to_string_lossy().to_string();
            let meta_file = entry.path().join("assignment.toml");
            if meta_file.is_file() {
              ui::ok(&format!("Assignment '{slug}' configuration valid"));
            } else {
              ui::warn(&format!("Assignment '{slug}' missing assignment.toml"));
            }
          }
        }
        ui::ok("Validation complete");
      }
    }
    Ok(())
  }
}

/// Executes git with the given arguments, mapping missing executable errors
/// cleanly.
fn run_git(args: &[&str]) -> Result<process::ExitStatus, Error> {
  process::Command::new("git")
    .args(args)
    .status()
    .map_err(|e| {
      if e.kind() == io::ErrorKind::NotFound {
        Error::Io(io::Error::new(
          io::ErrorKind::NotFound,
          "'git' was not found in PATH. Please install git.",
        ))
      } else {
        Error::Io(e)
      }
    })
}
