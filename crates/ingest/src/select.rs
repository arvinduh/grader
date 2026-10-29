//! Keeps the latest version of each file and names where it goes.
//!
//! Pure: turns `scan`'s students and submissions into manifest entries and
//! extraction tasks. Reading the zip belongs to `scan`; writing files belongs
//! to `extract`.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use chrono::NaiveDateTime;

use crate::find::Assignment;
use crate::model::{Student, ZipLocation};
use crate::{extract, manifest};

/// A selected file with its clean relative path and submission time.
struct SelectedFile<'a> {
  path: String,
  original: String,
  submitted_at: NaiveDateTime,
  location: &'a ZipLocation,
}

/// Picks each student's latest files, names their destinations, and builds
/// the manifest and extraction tasks.
///
/// Files are the same when their paths match case-insensitively after
/// dropping copy suffixes like `hw (2).ipynb`. Later attempts win; within an
/// attempt the higher copy number wins.
///
/// A student with one file gets `<slug>.<ext>`; otherwise files keep their
/// relative paths under `<slug>/`. Colliding slugs get `_<user id>` appended,
/// and an empty slug falls back to the user id.
#[must_use]
pub fn select(
  assignment: Assignment,
  students: &[Student],
) -> (manifest::Manifest, Vec<extract::Task>) {
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

  let mut manifest_students = BTreeMap::new();
  let mut tasks = Vec::new();

  for ((student, files), slug) in student_files.into_iter().zip(&slugs) {
    let user_id = student.id.to_string();
    let base_name = match slug_counts.get(slug.as_str()).copied().unwrap_or(0) {
      _ if slug.is_empty() => user_id,
      1 => slug.clone(),
      _ => format!("{slug}_{user_id}"),
    };

    let paths = destination_paths(&base_name, &files);
    let is_single = files.len() == 1;

    let mut selected = BTreeMap::new();
    for (file, path) in files.into_iter().zip(paths) {
      let new_name = if is_single {
        path.clone()
      } else {
        file.path.clone()
      };

      selected.insert(
        new_name,
        manifest::SelectedFile {
          original: file.original,
          submitted_at: file.submitted_at,
        },
      );

      tasks.push(extract::Task {
        destination: path,
        location: file.location.clone(),
      });
    }

    let submissions = student
      .submissions
      .iter()
      .map(|sub| manifest::SubmissionRecord {
        submitted_at: sub.submitted_at,
        comment: sub.comment.clone(),
        files: sub.files.iter().map(|f| f.rel_path.clone()).collect(),
      })
      .collect();

    manifest_students.insert(
      base_name,
      manifest::StudentEntry {
        id: student.id,
        name: student.name.clone(),
        submissions,
        selected,
      },
    );
  }

  (
    manifest::Manifest {
      assignment,
      students: manifest_students,
    },
    tasks,
  )
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
              original: file.rel_path.clone(),
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
                original: file.rel_path.clone(),
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

/// Splits a browser copy suffix off a path's file name.
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
