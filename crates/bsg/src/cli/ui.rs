//! Clean terminal UI with ANSI colors, status symbols, and in-place progress.
//!
//! Provides minimal, human-readable terminal output without emojis or logging
//! clutter.

use std::io::{self, Write};

pub const COLOR_GREEN: &str = "\x1b[32m";
pub const COLOR_YELLOW: &str = "\x1b[33m";
pub const COLOR_RED: &str = "\x1b[31m";
pub const COLOR_CYAN: &str = "\x1b[36m";
pub const COLOR_RESET: &str = "\x1b[0m";

pub const SYM_OK: &str = "✓";
pub const SYM_FAIL: &str = "✗";
pub const SYM_WARN: &str = "!";
pub const SYM_INFO: &str = "•";

const BRAILLE_FRAMES: &[&str] =
  &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Prints a green success message with `✓`.
pub fn ok(msg: &str) {
  println!("{COLOR_GREEN}{SYM_OK}{COLOR_RESET} {msg}");
}

/// Prints a yellow warning message with `!`.
pub fn warn(msg: &str) {
  println!("{COLOR_YELLOW}{SYM_WARN}{COLOR_RESET} {msg}");
}

/// Prints a red failure message with `✗`.
pub fn fail(msg: &str) {
  eprintln!("{COLOR_RED}{SYM_FAIL}{COLOR_RESET} {msg}");
}

/// Prints a cyan information message with `•`.
pub fn info(msg: &str) {
  println!("{COLOR_CYAN}{SYM_INFO}{COLOR_RESET} {msg}");
}

/// Lightweight in-place Braille spinner for multi-step tasks.
pub struct Spinner {
  frame_idx: usize,
}

impl Default for Spinner {
  fn default() -> Self {
    Self::new()
  }
}

impl Spinner {
  /// Creates a new spinner.
  #[must_use]
  pub fn new() -> Self {
    Self { frame_idx: 0 }
  }

  /// Updates the current terminal line with the next Braille frame.
  pub fn step(&mut self, msg: &str) {
    let frame = BRAILLE_FRAMES[self.frame_idx % BRAILLE_FRAMES.len()];
    self.frame_idx = self.frame_idx.wrapping_add(1);
    print!("\r\x1b[2K{COLOR_CYAN}{frame}{COLOR_RESET} {msg}");
    let _ = io::stdout().flush();
  }

  /// Clears the line and prints the final green success state.
  #[expect(
    clippy::unused_self,
    reason = "consumes spinner to terminate lifecycle"
  )]
  pub fn done(self, msg: &str) {
    print!("\r\x1b[2K{COLOR_GREEN}{SYM_OK}{COLOR_RESET} {msg}\n");
    let _ = io::stdout().flush();
  }
}
