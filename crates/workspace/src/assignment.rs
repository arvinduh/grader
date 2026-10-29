//! Assignment authoring metadata and TA group definitions.
//!
//! Owns parsing and matching `.bsg/assignments/<slug>/assignment.toml`.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Group assignment representation: an alphabetical range or explicit list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum GroupDefinition {
  /// Range of student last names, e.g. "Adebayo..Luu".
  Range(String),
  /// Explicit list of student slugs, e.g. `["adebayo_elizabeth", ...]`.
  List(Vec<String>),
}

impl GroupDefinition {
  /// Checks whether a given student belongs to this group.
  #[must_use]
  pub fn matches(&self, slug: &str, last_name: &str) -> bool {
    match self {
      Self::List(slugs) => slugs.iter().any(|s| s.eq_ignore_ascii_case(slug)),
      Self::Range(range) => {
        let parts: Vec<&str> = range.split("..").collect();
        if parts.len() != 2 {
          return false;
        }
        let start = parts[0].trim().to_lowercase();
        let end = parts[1].trim().to_lowercase();
        let target = last_name.trim().to_lowercase();

        target.as_str() >= start.as_str()
          && (target.as_str() <= end.as_str()
            || target.starts_with(end.as_str()))
      }
    }
  }
}

/// Metadata, deadlines, and group allocations for an assignment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AssignmentMeta {
  /// Canonical assignment name matching brightspace title.
  pub name: String,
  /// Alternate search names or CLI aliases.
  #[serde(default)]
  pub aliases: Vec<String>,
  /// Deadline timestamp.
  #[serde(default)]
  pub due: Option<NaiveDateTime>,
  /// Allocated TA grading groups.
  #[serde(default)]
  pub groups: BTreeMap<String, GroupDefinition>,
}

impl AssignmentMeta {
  /// Reads assignment metadata from a TOML file.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if reading or parsing fails.
  pub fn read_from(path: &Path) -> io::Result<Self> {
    let content = fs::read_to_string(path)?;
    toml::from_str(&content).map_err(io::Error::other)
  }

  /// Writes assignment metadata to a TOML file.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if writing fails.
  pub fn write_to(&self, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(self).map_err(io::Error::other)?;
    fs::write(path, content)
  }

  /// Checks if a candidate archive name matches this assignment or any alias.
  #[must_use]
  pub fn matches_candidate(&self, candidate: &str) -> bool {
    if self.name.eq_ignore_ascii_case(candidate) {
      return true;
    }
    self
      .aliases
      .iter()
      .any(|alias| alias.eq_ignore_ascii_case(candidate))
  }
}
