//! Writes selected files out of the download archive.
//!
//! Executes decisions only: which files and where come from `select`.

use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use zip::ZipArchive;
use zip::result::ZipError;

use crate::model::ZipLocation;

/// A file to extract from the archive into the output directory.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Task {
  /// Destination path relative to output directory, `/`-separated.
  pub destination: String,
  /// Where the file sits in the archive.
  pub location: ZipLocation,
}

/// Copies every file in `tasks` from `zip` to its path under `out`.
///
/// Files are written in origin order, so each nested zip is decompressed
/// once however many files come from it.
///
/// # Errors
///
/// Fails on the first file that cannot be read or written; files written
/// before it stay on disk.
///
/// # Side Effects
///
/// Creates `out` and directories under it; overwrites existing files there.
pub fn extract<R: Read + Seek>(
  zip: &mut ZipArchive<R>,
  tasks: &[Task],
  out: &Path,
) -> Result<(), ZipError> {
  let mut sorted: Vec<&Task> = tasks.iter().collect();
  sorted.sort_by_key(|t| &t.location);

  let mut nested: Option<(&str, ZipArchive<Cursor<Vec<u8>>>)> = None;
  let mut last_parent: Option<PathBuf> = None;

  for task in sorted {
    let mut dest = out.to_path_buf();
    dest.extend(task.destination.split('/'));
    if let Some(parent) = dest.parent()
      && last_parent.as_deref() != Some(parent)
    {
      fs::create_dir_all(parent)?;
      last_parent = Some(parent.to_path_buf());
    }
    match &task.location {
      ZipLocation::Direct { entry } => {
        let mut writer = io::BufWriter::new(File::create(&dest)?);
        io::copy(&mut zip.by_name(entry)?, &mut writer)?;
      }
      ZipLocation::Nested {
        zip_entry,
        inner_path,
      } => {
        let mut archive = match nested.take() {
          Some((cached, archive)) if cached == zip_entry.as_str() => archive,
          _ => {
            let mut bytes = Vec::new();
            zip.by_name(zip_entry)?.read_to_end(&mut bytes)?;
            ZipArchive::new(Cursor::new(bytes))?
          }
        };
        let mut writer = io::BufWriter::new(File::create(&dest)?);
        io::copy(&mut archive.by_name(inner_path)?, &mut writer)?;
        nested = Some((zip_entry.as_str(), archive));
      }
    }
  }

  Ok(())
}
