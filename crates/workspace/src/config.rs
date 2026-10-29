//! Workspace configuration models and deserialization.
//!
//! Owns parsing and serializing `.bsg/config.toml`.

use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::{Deserialize, Serialize};

/// Course identification metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CourseConfig {
  /// Display name of the course (e.g. "TAC 459").
  pub name: String,
  /// Semester term string (e.g. "Spring 2026").
  pub semester: String,
}

impl Default for CourseConfig {
  fn default() -> Self {
    Self {
      name: "TAC 459".to_string(),
      semester: "Spring 2026".to_string(),
    }
  }
}

/// Workspace filesystem paths relative to workspace root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PathsConfig {
  /// Subdirectory holding raw Brightspace bulk download zip files.
  pub inputs: PathBuf,
  /// Subdirectory or root where extracted assignment directories live.
  pub assignments: PathBuf,
}

impl Default for PathsConfig {
  fn default() -> Self {
    Self {
      inputs: PathBuf::from("archives"),
      assignments: PathBuf::from("."),
    }
  }
}

/// Rules governing submission deadlines and penalty calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LatePolicyConfig {
  /// Total number of late passes allocated to each student at start of
  /// semester.
  pub initial_passes: u32,
  /// Duration in days covered by a single late pass.
  pub pass_duration_days: u32,
  /// Fractional score penalty (0.0 to 1.0) applied per penalty period.
  pub penalty_per_period: f64,
  /// Grace period in minutes after due date before delay is counted.
  pub grace_period_mins: u32,
  /// Maximum fractional score penalty cap (e.g. 1.0 for 100%).
  pub max_penalty: f64,
}

impl Default for LatePolicyConfig {
  fn default() -> Self {
    Self {
      initial_passes: 4,
      pass_duration_days: 3,
      penalty_per_period: 0.25,
      grace_period_mins: 15,
      max_penalty: 1.0,
    }
  }
}

/// Nested policy groupings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PolicyConfig {
  /// Late submission and pass deduction rules.
  pub late: LatePolicyConfig,
}

/// Root workspace configuration stored at `.bsg/config.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Config {
  /// Course identification.
  pub course: CourseConfig,
  /// Directory layout paths.
  #[serde(default)]
  pub paths: PathsConfig,
  /// Semester course policies.
  #[serde(default)]
  pub policy: PolicyConfig,
}

impl Config {
  /// Reads configuration from a TOML file.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if reading or parsing the file fails.
  pub fn read_from(path: &Path) -> io::Result<Self> {
    let content = fs::read_to_string(path)?;
    toml::from_str(&content).map_err(io::Error::other)
  }

  /// Writes configuration to a TOML file.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if serializing or writing to disk fails.
  pub fn write_to(&self, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(self).map_err(io::Error::other)?;
    fs::write(path, content)
  }
}
