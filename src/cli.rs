//! CLI command definitions, routing, and shared configuration.

mod ingest;
#[cfg(feature = "update")]
mod update;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// CLI-level errors from subcommand dispatch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Ingest(#[from] grader::Error),
  #[cfg(feature = "update")]
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

  /// Increase logging verbosity (-v for INFO, -vv for DEBUG, -vvv for TRACE).
  #[arg(short, long, action = clap::ArgAction::Count, global = true)]
  verbose: u8,
}

/// Subcommands supported by `bsg`.
#[derive(clap::Subcommand)]
enum Command {
  Ingest(ingest::IngestArgs),
  #[cfg(feature = "update")]
  Update(update::UpdateArgs),
}

impl Cli {
  pub fn run(self) -> Result<(), Error> {
    self.init_logging();

    match self.command {
      Command::Ingest(args) => Ok(args.run()?),
      #[cfg(feature = "update")]
      Command::Update(args) => Ok(args.run()?),
    }
  }

  /// Configures tracing log output based on CLI verbosity and `RUST_LOG`.
  fn init_logging(&self) {
    let default_level = match self.verbose {
      0 => LevelFilter::WARN,
      1 => LevelFilter::INFO,
      2 => LevelFilter::DEBUG,
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
