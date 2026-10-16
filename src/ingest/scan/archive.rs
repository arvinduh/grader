//! Brightspace zip archive entry inspection and nested archive unpacking.

use std::io::Cursor;
use std::path::Component;
use std::path::Path;

use chrono::NaiveDateTime;
use thiserror;
use tracing;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::ingest::time;

/// Errors produced when inspecting submission folder paths.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// A folder does not follow `<id>-<assignment> - <name> - <date>`.
  #[error("unrecognized submission folder `{folder}`")]
  Folder { folder: String },

  /// An invalid timestamp was encountered.
  #[error(transparent)]
  Time(#[from] time::Error),
}

/// Physical storage address inside the Brightspace download archive.
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
pub struct SubmittedFile {
  /// The relative filename inside the submission (e.g. "hw2.ipynb").
  pub rel_path: String,
  /// Where the raw bytes are physically located in the archive.
  pub location: ZipLocation,
}

/// Parses Brightspace folder format: `<user_id>-<assignment_id> - <name> -
/// <date>`.
///
/// # Errors
///
/// Fails with [Error::Folder] if the folder format does not match, or
/// [Error::Time] if the timestamp is invalid.
pub fn parse_folder_name(
  folder: &str,
) -> Result<(u32, &str, NaiveDateTime), Error> {
  let parse = || {
    let (ids, rest) = folder.split_once(" - ")?;
    let (raw_name, time_str) = rest.rsplit_once(" - ")?;
    let (user_id_str, _) = ids.split_once('-')?;
    let user_id = user_id_str.parse::<u32>().ok()?;
    Some((user_id, raw_name, time_str))
  };

  let (user_id, raw_name, time_str) = parse().ok_or_else(|| Error::Folder {
    folder: folder.to_owned(),
  })?;
  let submitted_at = time::parse(time_str)?;
  Ok((user_id, raw_name, submitted_at))
}

/// Lists a nested zip's files as they should appear in the attempt.
///
/// # Errors
///
/// Fails with [ZipError] if the nested zip payload is corrupt.
pub fn nested(
  bytes: Vec<u8>,
  dir: &str,
  zip_entry: &str,
) -> Result<Vec<SubmittedFile>, ZipError> {
  let mut zip = ZipArchive::new(Cursor::new(bytes))?;
  let mut found: Vec<(String, String)> = Vec::new();

  for index in 0..zip.len() {
    let inner = zip.by_index_raw(index)?;
    if inner.is_dir() {
      continue;
    }
    let enclosed = inner.enclosed_name();
    match enclosed.as_deref().and_then(split) {
      Some(parts) if !is_junk(&parts) => {
        found.push((parts.join("/"), inner.name().to_owned()));
      }
      Some(_) => {}
      None => tracing::warn!(name = inner.name(), "Skipping unsafe entry name"),
    }
  }

  let wrapper = found.first().and_then(|(path, _)| {
    let prefix = &path[..=path.find('/')?];
    let shared = found.iter().all(|(path, _)| path.starts_with(prefix));
    shared.then_some(prefix.len())
  });
  let strip = wrapper.unwrap_or(0);

  Ok(
    found
      .into_iter()
      .map(|(path, inner)| SubmittedFile {
        rel_path: format!("{dir}{}", &path[strip..]),
        location: ZipLocation::Nested {
          zip_entry: zip_entry.to_owned(),
          inner_path: inner,
        },
      })
      .collect(),
  )
}

/// Splits a sanitized entry path into UTF-8 components.
pub fn split(path: &Path) -> Option<Vec<&str>> {
  path
    .components()
    .filter(|part| *part != Component::CurDir)
    .map(|part| match part {
      Component::Normal(part) => part.to_str(),
      _ => None,
    })
    .collect()
}

/// Reports whether a path is OS, editor, or tooling debris not worth grading.
pub fn is_junk(parts: &[&str]) -> bool {
  let hidden = parts.iter().any(|part| {
    part.starts_with('.')
      || part.starts_with("~$")
      || matches!(*part, "__MACOSX" | "__pycache__")
  });
  let windows = parts.last().is_some_and(|name| {
    name.eq_ignore_ascii_case("thumbs.db")
      || name.eq_ignore_ascii_case("desktop.ini")
  });
  hidden || windows
}

/// Reports whether a path names a `.zip` file, ignoring case.
pub fn has_zip_extension(path: &str) -> bool {
  Path::new(path)
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}
