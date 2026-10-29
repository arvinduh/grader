//! Brightspace timestamp parsing.
//!
//! Brightspace formats clock times without a colon (e.g. `1210 AM` or `407
//! PM`). This module normalizes the timestamp into standard `%I:%M %p` format
//! before parsing.

use chrono::{NaiveDateTime, ParseError};

/// An error encountered while parsing a Brightspace timestamp.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("invalid timestamp `{text}`: {source}")]
pub struct Error {
  /// The input text that failed to parse.
  pub text: String,
  /// The underlying parse failure.
  #[source]
  pub source: ParseError,
}

/// Parses a Brightspace timestamp such as `Sep 26, 2026 1210 AM`.
///
/// # Errors
///
/// Fails with [Error] if the text does not match Brightspace's date/time
/// layout.
pub fn parse(text: &str) -> Result<NaiveDateTime, Error> {
  let normalized = text.rsplit_once(' ').and_then(|(rest, meridiem)| {
    let (date, clock) = rest.rsplit_once(' ')?;
    let split = clock.len().checked_sub(2)?;
    let (hour, minute) = (clock.get(..split)?, clock.get(split..)?);
    Some(format!("{date} {hour}:{minute} {meridiem}"))
  });
  NaiveDateTime::parse_from_str(
    normalized.as_deref().unwrap_or(text),
    "%b %d, %Y %I:%M %p",
  )
  .map_err(|source| Error {
    text: text.to_owned(),
    source,
  })
}
