//! Validation and conflict checking for assignment ingestion.
//!
//! Checks input archives for duplicates and validates output directories
//! against existing manifests before any extraction work begins. Reading
//! archives belongs to `scan`; extracting files belongs to `extract`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use chrono::NaiveDateTime;
use tracing;

use crate::ingest::find::Assignment;
use crate::ingest::manifest::Manifest;

/// State of an assignment's destination directory relative to an incoming
/// download.
pub enum DestinationState {
  /// Directory does not exist, or exists and is completely empty.
  Fresh,

  /// Manifest exists with an identical download timestamp.
  UpToDate { downloaded: NaiveDateTime },

  /// Manifest exists but has a different download timestamp.
  DateMismatch {
    existing: NaiveDateTime,
    incoming: NaiveDateTime,
  },

  /// Directory exists and is non-empty, but lacks a valid manifest.
  NonEmptyWithoutManifest,
}

/// The diagnosed state of discovered input archives.
pub struct InputDiagnosis {
  /// Unambiguous assignments with exactly one archive, ready for ingestion.
  pub ready: BTreeMap<PathBuf, Assignment>,

  /// Assignments with multiple conflicting archives, grouped by assignment
  /// name and sorted chronologically from oldest to newest.
  pub duplicates: BTreeMap<String, Vec<(PathBuf, Assignment)>>,
}

/// Diagnoses discovered archives, partitioning them into ready assignments and
/// conflicting duplicate sets sorted chronologically from oldest to newest.
pub fn inputs(archives: BTreeMap<PathBuf, Assignment>) -> InputDiagnosis {
  let mut grouped: BTreeMap<String, Vec<(PathBuf, Assignment)>> =
    BTreeMap::new();
  for (path, assignment) in archives {
    grouped
      .entry(assignment.name.clone())
      .or_default()
      .push((path, assignment));
  }

  let mut ready = BTreeMap::new();
  let mut duplicates = BTreeMap::new();
  for (name, mut list) in grouped {
    if list.len() > 1 {
      list.sort_by_key(|(_, a)| a.downloaded);
      duplicates.insert(name, list);
    } else if let Some((path, assignment)) = list.into_iter().next() {
      ready.insert(path, assignment);
    }
  }

  InputDiagnosis { ready, duplicates }
}

/// Inspects the state of an assignment's target output directory.
///
/// # Errors
///
/// Returns [io::Error] on I/O failures inspecting the directory.
pub fn inspect_destination(
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
        tracing::warn!(
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

/// Checks the destination directory against conflicts, logging warnings and
/// preparing it for extraction.
///
/// Returns `Ok(true)` if extraction should proceed, or `Ok(false)` if it
/// should be skipped.
///
/// # Errors
///
/// Returns [io::Error] if inspecting or wiping the directory fails.
pub fn destination(
  assignment: &Assignment,
  output_dir: &Path,
  manifest_filename: &str,
  force: bool,
) -> io::Result<bool> {
  let state = inspect_destination(assignment, output_dir, manifest_filename)?;

  match state {
    DestinationState::Fresh => Ok(true),
    DestinationState::UpToDate { downloaded } => {
      tracing::warn!(
        assignment = %assignment.name,
        output_dir = %output_dir.display(),
        %downloaded,
        "Assignment destination is already up to date with identical download \
         date"
      );
      if !force {
        return Ok(false);
      }
      tracing::info!(
        output_dir = %output_dir.display(),
        "Removing existing directory before force re-extraction"
      );
      fs::remove_dir_all(output_dir)?;
      Ok(true)
    }
    DestinationState::DateMismatch { existing, incoming } => {
      tracing::warn!(
        assignment = %assignment.name,
        output_dir = %output_dir.display(),
        %existing,
        %incoming,
        "Assignment destination already exists with different download date"
      );
      if !force {
        return Ok(false);
      }
      tracing::info!(
        output_dir = %output_dir.display(),
        "Removing existing directory before force re-extraction"
      );
      fs::remove_dir_all(output_dir)?;
      Ok(true)
    }
    DestinationState::NonEmptyWithoutManifest => {
      tracing::warn!(
        assignment = %assignment.name,
        output_dir = %output_dir.display(),
        "Assignment destination directory exists and is not empty without a \
         manifest"
      );
      if !force {
        return Ok(false);
      }
      tracing::info!(
        output_dir = %output_dir.display(),
        "Removing existing directory before force re-extraction"
      );
      fs::remove_dir_all(output_dir)?;
      Ok(true)
    }
  }
}
