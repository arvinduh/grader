//! Keeps the latest version of each file and names where it goes.
//!
//! Pure: turns `scan`'s students and submissions into manifest entries.
//! Reading the zip belongs to `scan`; writing files belongs to `extract`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::btree_map::Entry;
use std::path::Path;

use chrono::NaiveDateTime;

use crate::ingest::manifest;
use crate::ingest::manifest::Extracted;
use crate::ingest::scan::Student;
use crate::ingest::scan::ZipLocation;

/// A selected file with its clean relative path and submission time.
struct SelectedFile<'a> {
  path: String,
  submitted_at: NaiveDateTime,
  location: &'a ZipLocation,
}

/// Picks each student's latest files and names their destinations.
///
/// Files are the same when their paths match case-insensitively after
/// dropping copy suffixes like `hw (2).ipynb`. Later attempts win; within an
/// attempt the higher copy number wins.
///
/// A student with one file gets `<slug>.<ext>`; otherwise files keep their
/// relative paths under `<slug>/`. Colliding slugs get `_<user id>` appended,
/// and an empty slug falls back to the user id.
pub fn select(students: &[Student]) -> Vec<manifest::Entry> {
  // Select latest files for each student.
  let student_files: Vec<(&Student, Vec<SelectedFile<'_>>)> = students
    .iter()
    .map(|s| (s, select_files(s)))
    .filter(|(_, files)| !files.is_empty())
    .collect();

  // Count slug occurrences to handle collisions.
  let slugs: Vec<String> =
    student_files.iter().map(|(s, _)| s.name.slug()).collect();
  let mut slug_counts: HashMap<&str, usize> = HashMap::new();
  for slug in &slugs {
    *slug_counts.entry(slug.as_str()).or_default() += 1;
  }

  // Build manifest entries.
  student_files
    .into_iter()
    .zip(&slugs)
    .map(|((student, files), slug)| {
      let user_id = student.id.to_string();
      let base_name = match slug_counts.get(slug.as_str()).copied().unwrap_or(0)
      {
        _ if slug.is_empty() => user_id.clone(),
        1 => slug.clone(),
        _ => format!("{slug}_{user_id}"),
      };

      let paths = destination_paths(&base_name, &files);

      manifest::Entry {
        user_id,
        student: student.name.clone(),
        files: files
          .into_iter()
          .zip(paths)
          .map(|(file, path)| Extracted {
            path,
            submitted_at: file.submitted_at,
            origin: to_origin(file.location),
          })
          .collect(),
      }
    })
    .collect()
}

/// Picks the latest version of each unique file for a single student.
fn select_files(student: &Student) -> Vec<SelectedFile<'_>> {
  type Rank = (NaiveDateTime, u32, usize, usize);
  let mut kept: BTreeMap<String, (Rank, SelectedFile<'_>)> = BTreeMap::new();

  for (s, sub) in student.submissions.iter().enumerate() {
    for (f, file) in sub.files.iter().enumerate() {
      let (path, copy) = strip_copy(&file.rel_path);
      let rank = (sub.submitted_at, copy, s, f);
      let key = path.to_lowercase();

      match kept.entry(key) {
        Entry::Vacant(slot) => {
          slot.insert((
            rank,
            SelectedFile {
              path,
              submitted_at: sub.submitted_at,
              location: &file.location,
            },
          ));
        }
        Entry::Occupied(mut slot) => {
          if rank > slot.get().0 {
            slot.insert((
              rank,
              SelectedFile {
                path,
                submitted_at: sub.submitted_at,
                location: &file.location,
              },
            ));
          }
        }
      }
    }
  }

  kept.into_values().map(|(_, v)| v).collect()
}

/// Computes destination paths for a student's selected files.
fn destination_paths(
  base_name: &str,
  files: &[SelectedFile<'_>],
) -> Vec<String> {
  match files {
    [only] => {
      let mut name = base_name.to_owned();
      if let Some(ext) = Path::new(&only.path).extension() {
        name.push('.');
        name.push_str(&ext.to_string_lossy());
      }
      vec![name]
    }
    _ => files
      .iter()
      .map(|f| format!("{base_name}/{}", f.path))
      .collect(),
  }
}

/// Converts a [ZipLocation] into a manifest [manifest::Origin].
fn to_origin(location: &ZipLocation) -> manifest::Origin {
  match location {
    ZipLocation::Direct { entry } => manifest::Origin {
      entry: entry.clone(),
      inner: None,
    },
    ZipLocation::Nested {
      zip_entry,
      inner_path,
    } => manifest::Origin {
      entry: zip_entry.clone(),
      inner: Some(inner_path.clone()),
    },
  }
}

/// Splits a browser copy suffix off a path's file name.
///
/// Recognizes ` (N)` (Chrome, Edge) and `(N)` (Firefox) just before the
/// extension, returning the path without it and `N`, or the path and `0`.
fn strip_copy(path: &str) -> (String, u32) {
  let name_start = path.rfind('/').map_or(0, |i| i + 1);
  let ext_start = path[name_start..]
    .rfind('.')
    .filter(|&i| i > 0)
    .map_or(path.len(), |i| name_start + i);
  let (base, ext) = path.split_at(ext_start);
  let stripped = base
    .strip_suffix(')')
    .and_then(|base| base.rsplit_once('('))
    .and_then(|(head, digits)| {
      let head = head.trim_end();
      let numeric =
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
      if !numeric || head.len() <= name_start {
        return None;
      }
      Some((head, digits.parse().ok()?))
    });
  match stripped {
    Some((head, copy)) => (format!("{head}{ext}"), copy),
    None => (path.to_owned(), 0),
  }
}
