//! The `ingest` command: extracts the latest submissions from downloads.

use std::collections::BTreeMap;
use std::fs::File;
use std::path;

use grader::Error;
use grader::ingest;
use grader::ingest::manifest::Manifest;
use zip::ZipArchive;

#[derive(clap::Args)]
pub struct IngestArgs {
  /// Brightspace downloads, or directories whose downloads to ingest
  /// (subdirectories are not searched).
  #[arg(value_name = "TARGET", required = true)]
  pub targets: Vec<path::PathBuf>,

  /// Directory to extract into instead of beside each download.
  #[arg(long, short, value_name = "DIR")]
  pub output_dir: Option<path::PathBuf>,
}

/// A validated download archive ready for extraction.
struct ValidatedDownload {
  zip: ZipArchive<File>,
  manifest: Manifest,
  output_dir: path::PathBuf,
}

impl IngestArgs {
  pub fn run(self) -> Result<(), Error> {
    // Collect all valid assignments
    let mut assignments = BTreeMap::new();
    for target in &self.targets {
      ingest::find(target, &mut assignments)?;
    }

    // Validate and plan every download before writing anything to disk.
    let mut validated_downloads = Vec::new();
    for (path, assignment) in assignments {
      let file = File::open(&path)?;
      let mut zip = ZipArchive::new(file)?;
      let output_dir = match &self.output_dir {
        Some(dir) => dir.join(&assignment.name),
        None => path.with_file_name(&assignment.name),
      };
      let students = ingest::scan(&mut zip)?;
      let manifest = Manifest {
        assignment,
        students: ingest::select(&students),
      };
      validated_downloads.push(ValidatedDownload {
        zip,
        manifest,
        output_dir,
      });
    }

    // Execute extractions and write manifests now that all downloads are
    // validated.
    for mut download in validated_downloads {
      ingest::extract(
        &mut download.zip,
        &download.manifest,
        &download.output_dir,
      )?;
      let manifest_path = download.output_dir.join(ingest::MANIFEST_FILENAME);
      download.manifest.write_to(&manifest_path)?;
    }

    Ok(())
  }
}
