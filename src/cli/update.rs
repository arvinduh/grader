//! The `update` command: replaces the running binary with the latest release.
//!
//! Checks GitHub releases for a newer version and drives `axoupdater` to
//! download and replace the running binary in place.

use std::env;
use std::ffi;
use std::io;
use std::path;

use axoupdater::AxoUpdater;
use axoupdater::AxoupdateError;
use semver::Version;
use tempfile;
use tokio::runtime;

/// The cargo-dist package name that names release installer assets.
const DIST_APP: &str = env!("CARGO_PKG_NAME");

/// The executable binary name on disk.
const BIN_NAME: &str = env!("CARGO_BIN_NAME");

/// Why `bsg update` stopped.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The binary is not named `bsg`, so the installer would not replace it.
  #[error(
    "{} is not named {BIN_NAME}{}; rename it to update",
    path.display(),
    env::consts::EXE_SUFFIX
  )]
  NotNamedBsg { path: path::PathBuf },

  /// The binary's directory cannot be written.
  #[error(
    "cannot write to {}: {source}; rerun with permission to write there",
    dir.display()
  )]
  Unwritable {
    dir: path::PathBuf,
    source: io::Error,
  },

  /// The installer ran and failed.
  #[error(
    "the installer failed ({}) with no captured output; anything it printed \
     is above",
    status.map_or_else(
      || "killed by a signal".to_string(),
      |code| format!("exit status {code}")
    )
  )]
  InstallerFailed { status: Option<i32> },

  /// The current version is not valid SemVer.
  #[error("invalid current version: {0}")]
  Version(#[from] semver::Error),

  /// Resolving the release or running its installer failed.
  #[error(transparent)]
  Update(#[from] Box<AxoupdateError>),

  /// Locating the running binary or driving async tasks failed.
  #[error(transparent)]
  Io(#[from] io::Error),
}

/// Command-line arguments for `bsg update`.
#[derive(clap::Args)]
pub struct UpdateArgs {
  /// Only check for updates without installing.
  #[arg(long, short)]
  check: bool,
}

impl UpdateArgs {
  pub fn run(self) -> Result<(), Error> {
    let current = env!("CARGO_PKG_VERSION");
    let exe = running_exe()?;
    let mut updater = new_updater(&exe, current)?;
    let rt = runtime::Builder::new_current_thread()
      .enable_all()
      .build()?;

    let latest = match rt.block_on(updater.query_new_version()) {
      Ok(version) => version.map(ToString::to_string),
      Err(AxoupdateError::NoStableReleases { .. }) => None,
      Err(err) => return Err(Error::Update(Box::new(err))),
    };

    if self.check {
      match latest {
        Some(version) => {
          println!("bsg: v{current} -> v{version}");
        }
        None => {
          println!("bsg: up to date (v{current})");
        }
      }
      return Ok(());
    }

    let Some(latest) = latest else {
      println!("bsg: up to date (v{current})");
      return Ok(());
    };

    check_replaceable(&exe)?;

    println!("updating bsg: v{current} -> v{latest}");

    // Prevent cargo-dist installer from modifying shell/user PATH on
    // self-update. SAFETY: Single-threaded CLI entry point before worker
    // threads spawn.
    unsafe {
      env::set_var("GRADER_NO_MODIFY_PATH", "1");
      env::set_var("BSG_NO_MODIFY_PATH", "1");
    };

    match rt.block_on(updater.run()).map_err(|err| match err {
      AxoupdateError::InstallFailed {
        status,
        stdout: None,
        stderr: None,
      } => Error::InstallerFailed { status },
      err => Error::Update(Box::new(err)),
    })? {
      Some(_) => {
        println!("updated bsg to v{latest}");
        Ok(())
      }
      None => {
        println!("bsg: up to date (v{current})");
        Ok(())
      }
    }
  }
}

fn running_exe() -> io::Result<path::PathBuf> {
  let exe = env::current_exe()?;
  if cfg!(windows) {
    Ok(exe)
  } else {
    exe.canonicalize()
  }
}

fn install_dir(exe: &path::Path) -> &path::Path {
  exe.parent().unwrap_or_else(|| path::Path::new("."))
}

fn check_replaceable(exe: &path::Path) -> Result<(), Error> {
  let name = format!("{BIN_NAME}{}", env::consts::EXE_SUFFIX);
  let named =
    exe
      .file_name()
      .and_then(ffi::OsStr::to_str)
      .is_some_and(|actual| {
        actual == name || (cfg!(windows) && actual.eq_ignore_ascii_case(&name))
      });
  if !named {
    return Err(Error::NotNamedBsg {
      path: exe.to_path_buf(),
    });
  }
  let dir = install_dir(exe);
  tempfile::tempfile_in(dir).map_err(|source| Error::Unwritable {
    dir: dir.to_path_buf(),
    source,
  })?;
  Ok(())
}

fn new_updater(exe: &path::Path, current: &str) -> Result<AxoUpdater, Error> {
  let dir = install_dir(exe).to_str().ok_or_else(|| {
    io::Error::new(io::ErrorKind::InvalidData, "install path is not UTF-8")
  })?;
  let version = Version::parse(current)?;
  let mut session = AxoUpdater::new_for(DIST_APP);

  if let Some(token) = env::var("BSG_GITHUB_TOKEN")
    .ok()
    .or_else(|| env::var("GITHUB_TOKEN").ok())
  {
    session.set_github_token(&token);
  }

  session
    .set_release_source(axoupdater::ReleaseSource {
      release_type: axoupdater::ReleaseSourceType::GitHub,
      owner: "arvinduh".to_string(),
      name: "grader".to_string(),
      app_name: DIST_APP.to_string(),
    })
    .set_install_dir(dir)
    .set_current_version(version)
    .map_err(Box::new)?;
  Ok(session)
}
