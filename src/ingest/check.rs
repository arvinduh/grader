//! Validation and conflict checking for assignment ingestion.
//!
//! Checks input archives for duplicates and validates output directories
//! against existing manifests before any extraction work begins. Reading
//! archives belongs to `scan`; extracting files belongs to `extract`.

use std::fs;
use std::io;
use std::path::Path;

use chrono::NaiveDateTime;
use tracing;

use crate::ingest::find::Assignment;
use crate::ingest::manifest::Manifest;

/// State of an assignment's destination directory relative to an incoming
/// download.
#[derive(Debug)]
pub enum DestinationState {
  /// Directory does not exist, or exists and is completely empty.
  Fresh,

  /// Manifest exists with an identical download timestamp.
  UpToDate {
    /// Download timestamp recorded in the existing manifest.
    downloaded: NaiveDateTime,
  },

  /// Manifest exists but has a different download timestamp.
  DateMismatch {
    /// Download timestamp in the existing manifest.
    existing: NaiveDateTime,
    /// Download timestamp of the incoming archive.
    incoming: NaiveDateTime,
  },

  /// Directory exists and is non-empty, but lacks a valid manifest.
  NonEmptyWithoutManifest,
}

/// Partitioned diagnosis of discovered input assignment archives.
#[derive(Debug)]
pub struct SourceDiagnosis {
  /// Unambiguous assignments with exactly one archive, ready for ingestion.
  pub ready: Vec<Assignment>,

  /// Conflicting duplicate archives grouped by assignment name, sorted
  /// chronologically from oldest to newest.
  pub duplicates: Vec<Vec<Assignment>>,
}

/// Diagnoses discovered archives, partitioning them into ready assignments and
/// conflicting duplicate sets sorted chronologically from oldest to newest.
pub fn source(assignments: Vec<Assignment>) -> SourceDiagnosis {
  let mut assignments = assignments;
  assignments.sort_unstable_by(|a, b| {
    a.name
      .cmp(&b.name)
      .then_with(|| a.downloaded.cmp(&b.downloaded))
  });

  let mut ready = Vec::new();
  let mut duplicates = Vec::new();

  let mut current = assignments.into_iter().peekable();
  while let Some(first) = current.next() {
    if current.peek().is_some_and(|next| next.name == first.name) {
      let mut group = vec![first];
      while current
        .peek()
        .is_some_and(|next| next.name == group[0].name)
      {
        group.push(current.next().unwrap());
      }
      duplicates.push(group);
    } else {
      ready.push(first);
    }
  }

  SourceDiagnosis { ready, duplicates }
}

/// Inspects the target destination directory and existing manifest for
/// conflicts.
///
/// # Errors
///
/// Returns [io::Error] on I/O failures reading the directory or manifest.
pub fn destination(
  assignment: &Assignment,
  output_dir: &Path,
  manifest_filename: &str,
) -> io::Result<DestinationState> {
  if !output_dir.exists() {
    return Ok(DestinationState::Fresh);
  }

  let manifest_path = output_dir.join(manifest_filename);
  if manifest_path.exists() {
    match Manifest::read_from(&manifest_path) {
      Ok(existing) => {
        if existing.assignment.downloaded == assignment.downloaded {
          return Ok(DestinationState::UpToDate {
            downloaded: assignment.downloaded,
          });
        }
        return Ok(DestinationState::DateMismatch {
          existing: existing.assignment.downloaded,
          incoming: assignment.downloaded,
        });
      }
      Err(err) => {
        tracing::debug!(
          assignment = %assignment.name,
          output_dir = %output_dir.display(),
          %err,
          "Failed to read existing manifest; treating directory as unmanaged"
        );
        return Ok(DestinationState::NonEmptyWithoutManifest);
      }
    }
  }

  let is_empty = fs::read_dir(output_dir)?.next().is_none();
  if is_empty {
    Ok(DestinationState::Fresh)
  } else {
    Ok(DestinationState::NonEmptyWithoutManifest)
  }
}
