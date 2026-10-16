//! Writes a manifest's files out of the download, then the manifest.
//!
//! Executes decisions only: which files and where come from `select`.

use std::fs;
use std::fs::File;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::path::Path;

use zip::ZipArchive;
use zip::result::ZipError;

use crate::ingest::manifest::Extracted;
use crate::ingest::manifest::Manifest;

/// Copies every file in `manifest` from `zip` to its path under `out`.
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
  manifest: &Manifest,
  out: &Path,
) -> Result<(), ZipError> {
  let mut files: Vec<&Extracted> =
    manifest.students.iter().flat_map(|s| &s.files).collect();
  files.sort_by(|a, b| {
    (&a.origin.entry, &a.origin.inner).cmp(&(&b.origin.entry, &b.origin.inner))
  });

  let mut nested: Option<(&str, ZipArchive<Cursor<Vec<u8>>>)> = None;
  for file in files {
    let mut dest = out.to_path_buf();
    dest.extend(file.path.split('/'));
    if let Some(parent) = dest.parent() {
      fs::create_dir_all(parent)?;
    }
    let entry = file.origin.entry.as_str();
    let Some(inner) = &file.origin.inner else {
      io::copy(&mut zip.by_name(entry)?, &mut File::create(&dest)?)?;
      continue;
    };
    let mut archive = match nested.take() {
      Some((cached, archive)) if cached == entry => archive,
      _ => {
        let mut bytes = Vec::new();
        zip.by_name(entry)?.read_to_end(&mut bytes)?;
        ZipArchive::new(Cursor::new(bytes))?
      }
    };
    io::copy(&mut archive.by_name(inner)?, &mut File::create(&dest)?)?;
    nested = Some((entry, archive));
  }

  Ok(())
}
