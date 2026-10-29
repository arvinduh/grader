//! CLI command definitions, routing, and shared configuration.

mod grade;
mod ingest;
mod repo;
mod status;
pub(crate) mod ui;
#[cfg(feature = "update")]
mod update;

use std::io;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// CLI-level errors from subcommand dispatch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// File system or I/O error.
  #[error(transparent)]
  Io(#[from] io::Error),
  /// Zip compression or format failure.
  #[error(transparent)]
  Zip(#[from] zip::result::ZipError),
  /// Archive search and discovery failure.
  #[error(transparent)]
  Find(#[from] bsg_ingest::find::Error),
  /// Archive scanning failure.
  #[error(transparent)]
  Scan(#[from] bsg_ingest::scan::Error),
  /// Ingestion engine error.
  #[error(transparent)]
  Ingest(#[from] bsg_ingest::Error),
  #[cfg(feature = "update")]
  /// Self-update error.
  #[error(transparent)]
  Update(#[from] update::Error),
}

/// Command line arguments for the `bsg` tool.
#[derive(clap::Parser)]
#[command(
  name = "bsg",
  author,
  version,
  about = "Util tool for grading Brightspace assignments",
  long_about = None
)]
pub struct Cli {
  #[command(subcommand)]
  command: Command,

  /// Increase logging verbosity (-v for WARN, -vv for INFO, -vvv for DEBUG).
  #[arg(short, long, action = clap::ArgAction::Count, global = true)]
  verbose: u8,
}

/// Subcommands supported by `bsg`.
#[derive(clap::Subcommand)]
enum Command {
  /// Ingest bulk download archives and update manifests.
  Ingest(ingest::IngestArgs),
  /// View submissions, late passes, and grading progress.
  Status(status::StatusArgs),
  /// Run tests and grade submissions with group slicing.
  Grade(grade::GradeArgs),
  /// Manage .bsg repository assets and synchronization.
  Repo(repo::RepoArgs),
  #[cfg(feature = "update")]
  /// Update the bsg binary.
  Update(update::UpdateArgs),
}

impl Cli {
  pub fn run(self) -> Result<(), Error> {
    self.init_logging();

    match self.command {
      Command::Ingest(args) => Ok(args.run()?),
      Command::Status(args) => Ok(args.run()?),
      Command::Grade(args) => Ok(args.run()?),
      Command::Repo(args) => Ok(args.run()?),
      #[cfg(feature = "update")]
      Command::Update(args) => Ok(args.run()?),
    }
  }

  /// Configures tracing log output based on CLI verbosity and `RUST_LOG`.
  fn init_logging(&self) {
    let default_level = match self.verbose {
      0 => LevelFilter::OFF,
      1 => LevelFilter::WARN,
      2 => LevelFilter::INFO,
      3 => LevelFilter::DEBUG,
      _ => LevelFilter::TRACE,
    };

    let filter = EnvFilter::builder()
      .with_default_directive(default_level.into())
      .from_env_lossy();

    let _ = tracing_subscriber::fmt()
      .with_env_filter(filter)
      .with_target(false)
      .without_time()
      .try_init();
  }
}
