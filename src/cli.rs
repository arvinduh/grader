//! CLI command definitions, routing, and shared configuration.
mod ingest;

use grader::Error;

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
}

impl Args {
  pub fn run(self) -> Result<(), Error> {
    match self {
      Self::Ingest(args) => args.run(),
    }
  }
}
