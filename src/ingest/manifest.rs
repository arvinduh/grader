//! In-memory and on-disk representation of an assignment manifest.
//!
//! Plain serializable types plus the student naming rule they share. Deciding
//! their contents is `select`'s job; writing them is `extract`'s.

use std::fs;
use std::io;
use std::path::Path;

use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;

use crate::ingest::find::Assignment;
use crate::ingest::scan::Name;

/// Every file extracted from one download, grouped by student.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
  #[serde(alias = "download")]
  pub assignment: Assignment,
  pub students: Vec<Entry>,
}

impl Manifest {
  /// Saves the manifest as JSON to the specified path.
  ///
  /// # Errors
  ///
  /// Fails on serialization or I/O failure.
  pub fn write_to(&self, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
    fs::write(path, json)
  }

  /// Reads a manifest from a JSON file at the specified path.
  ///
  /// # Errors
  ///
  /// Fails if the file cannot be read or deserialized.
  pub fn read_from(path: &Path) -> io::Result<Self> {
    let file = fs::File::open(path)?;
    let manifest = serde_json::from_reader(file).map_err(io::Error::other)?;
    Ok(manifest)
  }
}

/// One student's extracted files.
#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
  pub user_id: String,
  pub student: Name,
  pub files: Vec<Extracted>,
}

/// One extracted file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Extracted {
  /// Destination relative to the manifest, `/`-separated, without `..`.
  pub path: String,
  /// When the attempt holding this version was submitted.
  pub submitted_at: NaiveDateTime,
  /// Where a file sits in the download, by raw zip entry names.
  pub origin: Origin,
}

/// Where a file sits in the download, by raw zip entry names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Origin {
  /// The entry in the downloaded zip.
  pub entry: String,
  /// The member inside `entry` when `entry` is itself a zip.
  pub inner: Option<String>,
}
