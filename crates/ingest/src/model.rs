//! Data models representing scanned students, submissions, and files.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// A student's structured name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Name {
  /// First Name.
  pub first: String,
  /// Last Name.
  pub last: String,
}

impl Name {
  /// Creates a [Name] from a comma-separated string, e.g. "Last, First".
  ///
  /// Returns `None` if `name` does not contain `','`.
  #[must_use]
  pub fn from_comma(name: &str) -> Option<Self> {
    let (last, first) = name.split_once(',')?;
    Some(Self {
      first: first.trim().to_owned(),
      last: last.trim().to_owned(),
    })
  }

  /// Builds a filesystem-safe `last_first` name, e.g. `doe_john`.
  #[must_use]
  pub fn slug(&self) -> String {
    let name = self.last.chars().chain(['_']).chain(self.first.chars());
    let mut slug = String::new();
    for c in name {
      if c.is_alphanumeric() {
        slug.extend(c.to_lowercase());
      } else if !slug.is_empty() && !slug.ends_with('_') {
        slug.push('_');
      }
    }
    slug.truncate(slug.trim_end_matches('_').len());
    slug
  }
}

/// Physical storage address inside the Brightspace download archive.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ZipLocation {
  /// File sits directly in the download zip.
  Direct {
    /// Name of the zip entry.
    entry: String,
  },
  /// File sits inside a student-uploaded nested zip.
  Nested {
    /// Outer zip entry path.
    zip_entry: String,
    /// Path inside the nested zip.
    inner_path: String,
  },
}

/// A file uploaded by a student in a submission attempt.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SubmittedFile {
  /// The relative filename inside the submission (e.g. "hw2.ipynb").
  pub rel_path: String,
  /// Where the raw bytes are physically located in the archive.
  pub location: ZipLocation,
}

/// One submission attempt: everything a student uploaded at one time.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Submission {
  /// Local time as Brightspace printed it, to the minute.
  pub submitted_at: NaiveDateTime,
  /// Student-provided comment extracted from index.html for this attempt.
  pub comment: Option<String>,
  /// Files submitted in this attempt.
  pub files: Vec<SubmittedFile>,
}

/// One student's identity and their submission history.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Student {
  /// Unique Brightspace User ID.
  pub id: u32,
  /// Student name.
  pub name: Name,
  /// All submissions made by this student.
  pub submissions: Vec<Submission>,
}
