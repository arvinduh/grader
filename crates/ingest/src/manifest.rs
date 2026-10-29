//! In-memory and on-disk representation of an assignment manifest.
//!
//! Plain serializable types representing assignment metadata, student
//! submissions, and selected files. Deciding their contents is `select`'s job.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::find::Assignment;
use crate::model::Name;

/// In-memory and on-disk representation of an assignment manifest.
#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Manifest {
  /// Assignment metadata and download timestamp.
  #[serde(alias = "download")]
  pub assignment: Assignment,
  /// Map of student slugs to their submission records and selected files.
  pub students: BTreeMap<String, StudentEntry>,
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
    serde_json::from_reader(file).map_err(io::Error::other)
  }
}

/// One student's identity, submission attempts, and selected files.
#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StudentEntry {
  /// Unique Brightspace User ID.
  pub id: u32,
  /// Student name.
  pub name: Name,
  /// All submission attempts made by this student.
  pub submissions: Vec<SubmissionRecord>,
  /// Files selected for grading, keyed by their extracted relative name.
  pub selected: BTreeMap<String, SelectedFile>,
}

/// One submission attempt: everything a student uploaded at one time.
#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubmissionRecord {
  /// Local time as Brightspace printed it, to the minute.
  pub submitted_at: NaiveDateTime,
  /// Student-provided comment extracted from index.html for this attempt.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub comment: Option<String>,
  /// Files uploaded in this attempt, relative to the submission root.
  pub files: Vec<String>,
}

/// A selected file extracted to disk.
#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectedFile {
  /// Original relative filename submitted by the student.
  pub original: String,
  /// When the attempt holding this version was submitted.
  pub submitted_at: NaiveDateTime,
}
