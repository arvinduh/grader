//! Brightspace bulk assignment download ingestion pipeline.
//!
//! Owns discovering archives, scanning rosters and attempts, selecting latest
//! submissions, extracting files to disk, and generating `manifest.json`.

pub mod check;
pub mod extract;
pub mod find;
pub mod manifest;
pub mod model;
pub mod roster;
pub mod scan;
pub mod select;
pub mod time;

use std::io;

pub use check::{
  DestinationState, SourceDiagnosis, destination, source as check_src,
};
pub use extract::{Task, extract};
pub use find::{Assignment, find};
pub use manifest::{Manifest, SelectedFile, StudentEntry, SubmissionRecord};
pub use model::{Name, Student, Submission, SubmittedFile, ZipLocation};
pub use roster::Roster;
pub use scan::scan;
pub use select::select;
use zip::result::ZipError;

/// Every error ingesting can produce.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
  /// File system error.
  #[error(transparent)]
  Io(#[from] io::Error),
  /// Zip compression or format failure.
  #[error(transparent)]
  Zip(#[from] ZipError),
  /// Archive scanning failure.
  #[error(transparent)]
  Scan(#[from] scan::Error),
  /// Archive search and discovery failure.
  #[error(transparent)]
  Find(#[from] find::Error),
}
