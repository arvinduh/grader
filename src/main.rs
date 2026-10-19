//! The `bsg` binary entry point.

mod cli;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
  let args = cli::Cli::parse();
  match args.run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("[ERROR] {err}");
      ExitCode::FAILURE
    }
  }
}
