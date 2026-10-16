//! Discovers Brightspace bulk assignment downloads on disk.
//!
//! Inspects files and directories non-recursively, validating that candidate
//! archives match Brightspace naming conventions before passing them to the
//! scanner.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use chrono::NaiveDateTime;
use file_type_enum::FileType;
use tracing;

use crate::ingest::time;

/// Errors from parsing a Brightspace-generated name.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// IO Error.
  #[error(transparent)]
  Io(#[from] io::Error),

  /// A candidate download path is not a regular file.
  #[error("expected {path} to be a file, got a {file_type}")]
  FileType { path: PathBuf, file_type: FileType },

  /// A candidate download does not have a `.zip` extension.
  #[error("expected a `.zip` file, got {path}")]
  Extension { path: PathBuf },

  /// A download file name does not match `<assignment> Download <date>.zip`.
  #[error("expected `.../<assignment> Download <date>.zip` got `{path}`")]
  FileName { path: PathBuf },

  /// The timestamp encoded in the download file name could not be parsed.
  #[error(transparent)]
  Time(#[from] time::Error),
}

/// Represents an assignment's parsed metadata.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Assignment {
  /// The name of the assignment.
  pub name: String,
  /// The date and time when the assignment's archive was downloaded.
  pub downloaded: NaiveDateTime,
}

impl Assignment {
  /// Parses an assignment name and download date from a file path.
  ///
  /// Verifies that the path exists, is a regular file, has a `.zip` extension,
  /// and follows the Brightspace download naming convention:
  /// `<assignment> Download <Mon D, YYYY HMM AM>.zip`.
  ///
  /// # Errors
  ///
  /// Fails if the file does not exist, is not a regular `.zip` file, or
  /// contains an invalid timestamp.
  fn from_path(path: &Path) -> Result<Self, Error> {
    let file_type = FileType::symlink_read_at(path)?;

    if file_type != FileType::Regular {
      return Err(Error::FileType {
        path: path.to_path_buf(),
        file_type,
      });
    }

    if !path
      .extension()
      .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
      return Err(Error::Extension {
        path: path.to_path_buf(),
      });
    }

    let stem =
      path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| Error::FileName {
          path: path.to_path_buf(),
        })?;

    let (assignment, time) =
      stem
        .rsplit_once(" Download ")
        .ok_or_else(|| Error::FileName {
          path: path.to_path_buf(),
        })?;

    let downloaded = time::parse(time)?;

    Ok(Assignment {
      name: assignment.to_owned(),
      downloaded,
    })
  }
}

/// Finds all Brightspace assignment archives at the given target path.
///
/// If `target` is a regular file, validates and inserts it into `zip_paths`.
/// If `target` is a directory, inspects all top-level files non-recursively,
/// inserting valid assignment archives and logging invalid files at debug
/// level.
///
/// # Errors
///
/// Fails on I/O errors reading the directory or if `target` is an unsupported
/// file type (e.g. symlink).
pub fn find(
  target: &Path,
  zip_paths: &mut BTreeMap<PathBuf, Assignment>,
) -> Result<(), Error> {
  let file_type = FileType::symlink_read_at(target)?;

  match file_type {
    FileType::Regular => {
      let assignment = Assignment::from_path(target)?;
      zip_paths.insert(target.canonicalize()?, assignment);
      Ok(())
    }

    FileType::Directory => {
      for entry in fs::read_dir(target)? {
        let path = entry?.path();
        let assignment = Assignment::from_path(&path);
        let resolved = path.canonicalize()?;
        match assignment {
          Ok(ass) => {
            zip_paths.insert(resolved, ass);
          }
          Err(err) => {
            tracing::debug!(
              path = %resolved.display(),
              error = %err,
              "Skipping invalid assignment file"
            );
          }
        };
      }
      Ok(())
    }

    _ => Err(Error::FileType {
      path: target.canonicalize()?,
      file_type,
    }),
  }
}
