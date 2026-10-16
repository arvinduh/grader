//! Lists a download's students, submissions, and files without extracting
//! payloads.
//!
//! Owns reading `index.html` for ground-truth student names and comments,
//! scanning the zip directory for submission folders, expanding nested zips one
//! level, and dropping OS and editor junk. File selection and destination
//! naming belong to `select`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Seek;

use thiserror;
use tracing;
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

mod archive;
mod model;
mod roster;

pub use archive::SubmittedFile;
pub use archive::ZipLocation;
pub use model::Name;
pub use model::Student;
pub use model::Submission;
use roster::Roster;

/// Errors produced during download archive scanning.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// Underlying archive reading error.
  #[error(transparent)]
  Zip(#[from] ZipError),

  /// Underlying I/O error.
  #[error(transparent)]
  Io(#[from] io::Error),

  /// Unrecognized or invalid folder structure inside the archive.
  #[error(transparent)]
  Archive(#[from] archive::Error),

  /// A student name was not found in index.html.
  #[error("student `{name}` not found in index.html roster")]
  StudentNotFound { name: String },
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
///
/// Skips directory entries, unsafe paths, root-level files, and tooling junk.
fn classify_entry<R: Read>(entry: &ZipFile<'_, R>) -> Option<AttemptPath> {
  if entry.is_dir() {
    return None;
  }
  let enclosed = entry.enclosed_name();
  let Some(parts) = enclosed.as_deref().and_then(archive::split) else {
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
  if archive::is_junk(rest) {
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

  let (id, raw_name, submitted_at) = archive::parse_folder_name(folder)?;
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

  if archive::has_zip_extension(&rel_path) {
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(ZipError::Io)?;
    let dir = rel_path.rfind('/').map_or("", |i| &rel_path[..=i]);
    match archive::nested(bytes, dir, &entry_name) {
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
