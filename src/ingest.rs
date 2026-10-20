//! Turns Brightspace bulk downloads into one output per student.
//!
//! Steps a caller chains: `find` lists downloads; `check` validates sources
//! and destinations, `scan` lists attempts without extracting, `select` keeps
//! the latest version of each file and names it, and `extract` writes the files
//! and the `manifest::Manifest`.

use std::io;

use zip::result::ZipError;

mod check;
mod extract;
mod find;
mod manifest;
mod scan;
mod select;
mod time;

pub use check::DestinationState;
pub use check::SourceDiagnosis;
pub use check::destination as check_dest;
pub use check::source as check_src;
pub use extract::Task;
pub use extract::extract;
pub use find::Assignment;
pub use find::find;
pub use manifest::Manifest;
pub use manifest::SelectedFile;
pub use manifest::StudentEntry;
pub use manifest::SubmissionRecord;
pub use scan::Name;
pub use scan::Student;
pub use scan::scan;
pub use select::select;

/// Every error ingesting can produce.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error(transparent)]
  Zip(#[from] ZipError),
  #[error(transparent)]
  Scan(#[from] scan::Error),
  #[error(transparent)]
  Find(#[from] find::Error),
}
