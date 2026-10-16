//! CLI command definitions, routing, and shared configuration.

mod ingest;
#[cfg(feature = "update")]
mod update;

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
pub enum Args {
  Ingest(ingest::IngestArgs),
  #[cfg(feature = "update")]
  Update(update::UpdateArgs),
}

impl Args {
  pub fn run(self) -> Result<(), Error> {
    match self {
      Self::Ingest(args) => Ok(args.run()?),
      #[cfg(feature = "update")]
      Self::Update(args) => Ok(args.run()?),
    }
  }
}
