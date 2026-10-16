//! Data models representing scanned students, submissions, and files.

use chrono::NaiveDateTime;

use super::archive::SubmittedFile;

/// A student's structured name.
#[derive(
  Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
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
  pub fn from_comma(name: &str) -> Option<Self> {
    let (last, first) = name.split_once(',')?;
    Some(Self {
      first: first.trim().to_owned(),
      last: last.trim().to_owned(),
    })
  }

  /// Builds a filesystem-safe `last_first` name, e.g. `doe_john`.
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

/// One submission attempt: everything a student uploaded at one time.
pub struct Submission {
  /// Local time as Brightspace printed it, to the minute.
  pub submitted_at: NaiveDateTime,
  /// Student-provided comment extracted from index.html for this attempt.
  pub comment: Option<String>,
  /// Files submitted in this attempt.
  pub files: Vec<SubmittedFile>,
}

/// One student's identity and their submission history.
pub struct Student {
  /// Unique Brightspace User ID.
  pub id: u32,
  /// Student name.
  pub name: Name,
  /// All submissions made by this student.
  pub submissions: Vec<Submission>,
}
