//! The `ingest` command: extracts the latest submissions from downloads.

use std::fs;
use std::fs::File;
use std::path;

use grader::Error;
use grader::ingest;
use grader::ingest::DestinationState;
use grader::ingest::Manifest;
use tracing;
use zip::ZipArchive;

/// The manifest file name generated for ingested assignments.
const MANIFEST_FILENAME: &str = ".manifest.json";

#[derive(clap::Args)]
pub struct IngestArgs {
  /// Brightspace downloads, or directories whose downloads to ingest
  /// (subdirectories are not searched).
  #[arg(value_name = "TARGET", required = true)]
  targets: Vec<path::PathBuf>,

  /// Directory to extract into instead of beside each download.
  #[arg(long, short, value_name = "DIR")]
  output_dir: Option<path::PathBuf>,

  /// Overwrite existing output directories.
  #[arg(long, short)]
  force: bool,
}

/// A validated download archive ready for extraction.
struct ValidatedDownload {
  zip: ZipArchive<File>,
  manifest: Manifest,
  tasks: Vec<ingest::Task>,
  output_dir: path::PathBuf,
}

impl IngestArgs {
  pub fn run(self) -> Result<(), Error> {
    // Discover and resolve all unique assignment archives across targets.
    let assignments = ingest::find(&self.targets)?;

    // Diagnose input archives and warn on ambiguous duplicates.
    let diagnosis = ingest::check_src(assignments);
    for group in &diagnosis.duplicates {
      let oldest = &group.first().unwrap().path;
      let newest = &group.last().unwrap().path;
      tracing::warn!(
        assignment = %group[0].name,
        count = group.len(),
        oldest = %oldest.display(),
        newest = %newest.display(),
        "Multiple archives found for assignment; skipping ambiguous input"
      );
    }

    // Validate and plan every download before writing anything to disk.
    let mut validated_downloads = Vec::new();
    for assignment in diagnosis.ready {
      let output_dir = match &self.output_dir {
        Some(dir) => dir.join(&assignment.name),
        None => assignment.path.with_file_name(&assignment.name),
      };

      let state =
        ingest::check_dest(&assignment, &output_dir, MANIFEST_FILENAME)?;
      match state {
        DestinationState::Fresh => {}
        DestinationState::UpToDate { downloaded } => {
          tracing::warn!(
            assignment = %assignment.name,
            output_dir = %output_dir.display(),
            %downloaded,
            "Assignment destination is already up to date with identical \
             download date"
          );
        }
        DestinationState::DateMismatch { existing, incoming } => {
          tracing::warn!(
            assignment = %assignment.name,
            output_dir = %output_dir.display(),
            %existing,
            %incoming,
            "Assignment destination already exists with different download \
             date"
          );
        }
        DestinationState::NonEmptyWithoutManifest => {
          tracing::warn!(
            assignment = %assignment.name,
            output_dir = %output_dir.display(),
            "Assignment destination directory exists and is not empty without \
             a manifest"
          );
        }
      }

      if !matches!(state, DestinationState::Fresh) {
        if !self.force {
          continue;
        }
        tracing::info!(
          output_dir = %output_dir.display(),
          "Removing existing directory before force re-extraction"
        );
        fs::remove_dir_all(&output_dir)?;
      }

      let file = File::open(&assignment.path)?;
      let mut zip = ZipArchive::new(file)?;
      let students = ingest::scan(&mut zip)?;
      let (manifest, tasks) = ingest::select(assignment, &students);
      validated_downloads.push(ValidatedDownload {
        zip,
        manifest,
        tasks,
        output_dir,
      });
    }

    // Execute extractions and write manifests now that all downloads are
    // validated.
    for mut download in validated_downloads {
      ingest::extract(
        &mut download.zip,
        &download.tasks,
        &download.output_dir,
      )?;
      let manifest_path = download.output_dir.join(MANIFEST_FILENAME);
      download.manifest.write_to(&manifest_path)?;
    }

    Ok(())
  }
}
