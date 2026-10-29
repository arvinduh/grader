//! Lists a download's students, submissions, and files without extracting
//! payloads.
//!
//! Owns reading `index.html` for ground-truth student names and comments,
//! scanning the zip directory for submission folders, expanding nested zips one
//! level, and dropping OS and editor junk. File selection and destination
//! naming belong to `select`.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Cursor, Read, Seek};
use std::path::{Component, Path};

use chrono::NaiveDateTime;
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

use crate::model::{Student, Submission, SubmittedFile, ZipLocation};
use crate::roster::Roster;
use crate::time;

/// Errors produced during download archive scanning.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
  /// Underlying archive reading error.
  #[error(transparent)]
  Zip(#[from] ZipError),

  /// Underlying I/O error.
  #[error(transparent)]
  Io(#[from] io::Error),

  /// A folder does not follow `<id>-<assignment> - <name> - <date>`.
  #[error("unrecognized submission folder `{folder}`")]
  Folder {
    /// The unparseable folder name.
    folder: String,
  },

  /// An invalid timestamp was encountered.
  #[error(transparent)]
  Time(#[from] time::Error),

  /// A student name was not found in index.html.
  #[error("student `{name}` not found in index.html roster")]
  StudentNotFound {
    /// Student name.
    name: String,
  },
}

/// Lists every student, submission attempt, and file in a download.
///
/// # Errors
///
/// Fails on a corrupt download archive or an unparseable submission folder.
pub fn scan<R: Read + Seek>(
  zip: &mut ZipArchive<R>,
) -> Result<Vec<Student>, Error> {
  let mut roster = read_roster(zip)?;
  let mut students: BTreeMap<u32, Student> = BTreeMap::new();
  let mut by_folder: HashMap<String, (u32, usize)> = HashMap::new();

  for index in 0..zip.len() {
    let mut entry = zip.by_index(index)?;
    let Some(target) = classify_entry(&entry) else {
      continue;
    };

    let files = read_submitted_files(&mut entry, target.rel_path)?;
    let submission = ensure_submission(
      &target.folder,
      &mut roster,
      &mut students,
      &mut by_folder,
    )?;
    submission.files.extend(files);
  }

  Ok(students.into_values().collect())
}

/// Extracts and parses `index.html` from the root of the archive.
fn read_roster<R: Read + Seek>(
  zip: &mut ZipArchive<R>,
) -> Result<Roster, Error> {
  let mut index_entry = zip.by_name("index.html")?;
  let mut html = String::new();
  index_entry.read_to_string(&mut html)?;
  Ok(Roster::from_html(&html))
}

/// Identified folder and attempt-relative path for a file in the archive.
struct AttemptPath {
  folder: String,
  rel_path: String,
}

/// Validates and filters a zip entry, returning its attempt folder and relative
/// path.
fn classify_entry<R: Read>(entry: &ZipFile<'_, R>) -> Option<AttemptPath> {
  if entry.is_dir() {
    return None;
  }
  let enclosed = entry.enclosed_name();
  let Some(parts) = enclosed.as_deref().and_then(split) else {
    tracing::warn!(name = entry.name(), "Skipping unsafe entry name");
    return None;
  };
  let [folder, rest @ ..] = parts.as_slice() else {
    return None;
  };
  if rest.is_empty() {
    if *folder != "index.html" {
      tracing::warn!(name = folder, "Skipping file outside any attempt");
    }
    return None;
  }
  if is_junk(rest) {
    return None;
  }
  Some(AttemptPath {
    folder: (*folder).to_owned(),
    rel_path: rest.join("/"),
  })
}

/// Finds or creates the [Submission] for an attempt folder.
fn ensure_submission<'a>(
  folder: &str,
  roster: &mut Roster,
  students: &'a mut BTreeMap<u32, Student>,
  by_folder: &mut HashMap<String, (u32, usize)>,
) -> Result<&'a mut Submission, Error> {
  if let Some(&(id, idx)) = by_folder.get(folder) {
    return Ok(
      &mut students
        .get_mut(&id)
        .expect("student exists in map")
        .submissions[idx],
    );
  }

  let (id, raw_name, submitted_at) = parse_folder_name(folder)?;
  let name = roster
    .lookup(raw_name)
    .ok_or_else(|| Error::StudentNotFound {
      name: raw_name.to_owned(),
    })?;
  let comment = roster.take_comment(&name, submitted_at);

  let student = students.entry(id).or_insert_with(|| Student {
    id,
    name,
    submissions: Vec::new(),
  });

  student.submissions.push(Submission {
    submitted_at,
    comment,
    files: Vec::new(),
  });

  let idx = student.submissions.len() - 1;
  by_folder.insert(folder.to_owned(), (id, idx));
  Ok(
    student
      .submissions
      .last_mut()
      .expect("submission was just pushed"),
  )
}

/// Reads an entry's submitted file(s), expanding nested zips if present.
fn read_submitted_files<R: Read>(
  entry: &mut ZipFile<'_, R>,
  rel_path: String,
) -> Result<Vec<SubmittedFile>, ZipError> {
  let entry_name = entry.name().to_owned();

  if has_zip_extension(&rel_path) {
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(ZipError::Io)?;
    let dir = rel_path.rfind('/').map_or("", |i| &rel_path[..=i]);
    match nested(bytes, dir, &entry_name) {
      Ok(files) => return Ok(files),
      Err(err) => {
        tracing::warn!(%rel_path, %err, "Keeping unreadable nested zip as direct file");
      }
    }
  }

  Ok(vec![SubmittedFile {
    rel_path,
    location: ZipLocation::Direct { entry: entry_name },
  }])
}

/// Parses Brightspace folder format: `<user_id>-<assignment_id> - <name> -
/// <date>`.
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
#[must_use]
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
#[must_use]
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
#[must_use]
pub fn has_zip_extension(path: &str) -> bool {
  Path::new(path)
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}
