//! Turns Brightspace bulk downloads into one output per student.
//!
//! Steps a caller chains: `find` lists downloads; then `scan` lists its
//! attempts without extracting, `select` keeps the latest version of each
//! file and names it, and `extract` writes the files and the
//! `manifest::Manifest`.

pub mod check;
mod extract;
mod find;
pub mod manifest;
mod scan;
mod select;
mod time;

use std::io;

pub use extract::Task;
pub use extract::extract;
pub use find::find;
pub use scan::scan;
pub use select::select;
use zip::result::ZipError;

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
