//! Workspace auto-discovery, layout paths, and assignment bundle resolution.
//!
//! Owns discovering `.bsg/` by walking up parent directories and resolving
//! paths within the course workspace.

pub mod assignment;
pub mod config;

use std::path::{Path, PathBuf};
use std::{env, fs, io};

pub use assignment::{AssignmentMeta, GroupDefinition};
pub use config::{
  Config, CourseConfig, LatePolicyConfig, PathsConfig, PolicyConfig,
};

/// Name of the hidden configuration and state directory.
pub const BSG_DIR_NAME: &str = ".bsg";
/// Name of the main configuration file inside `.bsg/`.
pub const CONFIG_FILE_NAME: &str = "config.toml";
/// Name of the assignment metadata file.
pub const ASSIGNMENT_META_NAME: &str = "assignment.toml";
/// Name of the manifest file.
pub const MANIFEST_FILE_NAME: &str = "manifest.json";
/// Name of the ledger file.
pub const LEDGER_FILE_NAME: &str = "ledger.json";

/// Discovered workspace context.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Workspace {
  /// Root directory containing the `.bsg/` state folder.
  pub root: PathBuf,
  /// Canonical path to `.bsg/`.
  pub bsg_dir: PathBuf,
  /// Loaded workspace configuration.
  pub config: Config,
}

impl Workspace {
  /// Discovers a `.bsg/` workspace by walking up from the current working
  /// directory.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] with `NotFound` if no `.bsg/config.toml` is found.
  pub fn discover() -> io::Result<Self> {
    let current = env::current_dir()?;
    Self::discover_from(&current)
  }

  /// Discovers a workspace starting from a specific directory.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] with `NotFound` if reaching filesystem root without
  /// discovering `.bsg/`.
  pub fn discover_from(start: &Path) -> io::Result<Self> {
    let mut current = start.to_path_buf();
    loop {
      let candidate = current.join(BSG_DIR_NAME);
      let config_file = candidate.join(CONFIG_FILE_NAME);
      if config_file.is_file() {
        let config = Config::read_from(&config_file)?;
        return Ok(Self {
          root: current,
          bsg_dir: candidate,
          config,
        });
      }
      if !current.pop() {
        return Err(io::Error::new(
          io::ErrorKind::NotFound,
          "No .bsg workspace found in current or parent directories. Run 'bsg \
           repo init' to create one.",
        ));
      }
    }
  }

  /// Path to the course assignments bundle root: `.bsg/assignments/`.
  #[must_use]
  pub fn assignments_dir(&self) -> PathBuf {
    self.bsg_dir.join("assignments")
  }

  /// Path to a specific assignment bundle directory.
  #[must_use]
  pub fn assignment_dir(&self, slug: &str) -> PathBuf {
    self.assignments_dir().join(slug)
  }

  /// Path to an assignment's `manifest.json`.
  #[must_use]
  pub fn manifest_path(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join(MANIFEST_FILE_NAME)
  }

  /// Path to an assignment's `assignment.toml`.
  #[must_use]
  pub fn assignment_meta_path(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join(ASSIGNMENT_META_NAME)
  }

  /// Path to an assignment's `rubric.toml`.
  #[must_use]
  pub fn rubric_path(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join("rubric.toml")
  }

  /// Path to an assignment's `solution.ipynb`.
  #[must_use]
  pub fn solution_path(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join("solution.ipynb")
  }

  /// Path to an assignment's `starter.ipynb`.
  #[must_use]
  pub fn starter_path(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join("starter.ipynb")
  }

  /// Path to an assignment's test scripts directory.
  #[must_use]
  pub fn tests_dir(&self, slug: &str) -> PathBuf {
    self.assignment_dir(slug).join("tests")
  }

  /// Path to the semester late pass ledger.
  #[must_use]
  pub fn ledger_path(&self) -> PathBuf {
    self.bsg_dir.join(LEDGER_FILE_NAME)
  }

  /// Resolves an assignment slug from an archive name or alias.
  #[must_use]
  pub fn resolve_slug(&self, candidate: &str) -> Option<String> {
    let assignments_dir = self.assignments_dir();
    if !assignments_dir.is_dir() {
      return None;
    }

    let entries = fs::read_dir(assignments_dir).ok()?;
    for entry in entries.flatten() {
      let slug = entry.file_name().to_string_lossy().to_string();
      if slug.eq_ignore_ascii_case(candidate) {
        return Some(slug);
      }
      let meta_file = entry.path().join(ASSIGNMENT_META_NAME);
      if meta_file.is_file()
        && let Ok(meta) = AssignmentMeta::read_from(&meta_file)
        && meta.matches_candidate(candidate)
      {
        return Some(slug);
      }
    }
    None
  }

  /// Initializes a new `.bsg/` directory at the specified course root.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if filesystem operations fail.
  pub fn init(root: &Path) -> io::Result<Self> {
    let bsg_dir = root.join(BSG_DIR_NAME);
    fs::create_dir_all(&bsg_dir)?;
    fs::create_dir_all(bsg_dir.join("assignments"))?;

    let config = Config::default();
    config.write_to(&bsg_dir.join(CONFIG_FILE_NAME))?;

    // Write a .gitignore inside .bsg to protect student records from remote
    // leaks.
    let gitignore_content =
      "manifest.json\nledger.json\n.venv/\ncache/\n__pycache__/\n*.pyc\n";
    fs::write(bsg_dir.join(".gitignore"), gitignore_content)?;

    Ok(Self {
      root: root.to_path_buf(),
      bsg_dir,
      config,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_init_and_discover() {
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let ws =
      Workspace::init(temp.path()).expect("workspace init should succeed");
    assert!(ws.bsg_dir.is_dir());
    assert!(ws.config.paths.inputs.ends_with("archives"));

    let discovered =
      Workspace::discover_from(temp.path()).expect("discover should succeed");
    assert_eq!(discovered.root, temp.path());
  }
}
