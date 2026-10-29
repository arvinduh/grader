//! Parses ground-truth student names and submission comments from Brightspace
//! `index.html`.
//!
//! Brightspace exports an `index.html` table in bulk downloads with unambiguous
//! `Last, First` student headers and submission comments.

use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::NaiveDateTime;
use scraper::{ElementRef, Html, Selector};

use crate::model::Name;
use crate::time;

/// Selects table rows.
static TR_SEL: LazyLock<Selector> =
  LazyLock::new(|| Selector::parse("tr").expect("valid tr selector"));
/// Selects bold text.
static B_SEL: LazyLock<Selector> =
  LazyLock::new(|| Selector::parse("b").expect("valid b selector"));
/// Selects table cells.
static TD_SEL: LazyLock<Selector> =
  LazyLock::new(|| Selector::parse("td").expect("valid td selector"));
/// Selects paragraphs.
static P_SEL: LazyLock<Selector> =
  LazyLock::new(|| Selector::parse("p").expect("valid p selector"));

/// Ground-truth roster and submission notes parsed from `index.html`.
pub struct Roster {
  /// Maps lowercase natural names (`"first last"`) to canonical [Name]s.
  natural_to_name: HashMap<String, Name>,
  /// Maps attempt identifiers to their submission comment.
  comments: HashMap<(Name, NaiveDateTime), String>,
}

impl Roster {
  /// Parses ground truth student names and attempt comments from `index.html`.
  ///
  /// The Brightspace table structure alternates between:
  /// - Student header rows (`bgcolor="#AAAAAA"`), containing `<b>Last,
  ///   First</b>`
  /// - Attempt rows (`bgcolor="white"`), containing comments and dates
  #[must_use]
  pub fn from_html(html: &str) -> Self {
    let document = Html::parse_document(html);
    let mut natural_to_name = HashMap::new();
    let mut comments = HashMap::new();
    let mut current_name: Option<Name> = None;

    for tr in document.select(&TR_SEL) {
      let bgcolor = tr.value().attr("bgcolor").unwrap_or_default();

      if bgcolor.eq_ignore_ascii_case("#AAAAAA") {
        if let Some(name) = parse_student_header(tr) {
          let natural = format!("{} {}", name.first, name.last).to_lowercase();
          natural_to_name.insert(natural, name.clone());
          current_name = Some(name);
        }
      } else if bgcolor.eq_ignore_ascii_case("white") {
        let Some(ref name) = current_name else {
          continue;
        };
        if let Some((submitted_at, Some(comment))) = parse_attempt_row(tr) {
          comments.insert((name.clone(), submitted_at), comment);
        }
      }
    }

    Self {
      natural_to_name,
      comments,
    }
  }

  /// Resolves lowercase natural names (`"first last"`) to canonical [Name]s
  /// from index.html.
  ///
  /// Returns `None` if the student name does not appear in index.html.
  #[must_use]
  pub fn lookup(&self, name: &str) -> Option<Name> {
    let key = name.trim().to_lowercase();
    self.natural_to_name.get(&key).cloned()
  }

  /// Takes any comment associated with a student attempt, moving it out of the
  /// roster.
  pub fn take_comment(
    &mut self,
    name: &Name,
    submitted_at: NaiveDateTime,
  ) -> Option<String> {
    self.comments.remove(&(name.clone(), submitted_at))
  }
}

/// Extracts a student's canonical [Name] from a header row
/// (`bgcolor="#AAAAAA"`).
fn parse_student_header(tr: ElementRef<'_>) -> Option<Name> {
  let b = tr.select(&B_SEL).next()?;
  Name::from_comma(&b.text().collect::<String>())
}

/// Parses an attempt timestamp and optional submission comment from a row
/// (`bgcolor="white"`).
fn parse_attempt_row(
  tr: ElementRef<'_>,
) -> Option<(NaiveDateTime, Option<String>)> {
  let mut tds = tr.select(&TD_SEL);
  let time_str = tds.next()?.text().collect::<String>();
  let submitted_at = time::parse(time_str.trim()).ok()?;
  let comment = tds
    .next()
    .and_then(|td| td.select(&P_SEL).next())
    .map(|p| p.text().collect::<String>())
    .filter(|s| !s.is_empty());
  Some((submitted_at, comment))
}
