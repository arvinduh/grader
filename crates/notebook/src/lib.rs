//! Jupyter notebook parsing, sanitization, and boilerplate stripping.
//!
//! Provides minimal serde mapping of nbformat 4 and cell-level set-intersection
//! stripping to reduce token context for AI grading.

use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Normalized representation of cell source lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum CellSource {
  /// Multi-line string represented as an array of line strings.
  Lines(Vec<String>),
  /// Single contiguous string block.
  Single(String),
}

impl CellSource {
  /// Converts the source to a single concatenated string.
  #[must_use]
  pub fn to_string_content(&self) -> String {
    match self {
      Self::Lines(lines) => lines.join(""),
      Self::Single(text) => text.clone(),
    }
  }
}

/// A single cell output item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CellOutput {
  /// Output MIME type or category (e.g. `stream`, `execute_result`, `error`).
  pub output_type: String,
  /// MIME-keyed data bundle for rich display outputs.
  #[serde(default)]
  pub data: Option<Map<String, Value>>,
  /// Stream text output lines or block.
  #[serde(default)]
  pub text: Option<CellSource>,
  /// Error exception name if cell execution failed.
  #[serde(default)]
  pub ename: Option<String>,
  /// Error exception value if cell execution failed.
  #[serde(default)]
  pub evalue: Option<String>,
}

/// A Jupyter notebook cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Cell {
  /// Cell type (`code`, `markdown`, or `raw`).
  pub cell_type: String,
  /// Source content lines or block.
  pub source: CellSource,
  /// Output items for code cells.
  #[serde(default)]
  pub outputs: Option<Vec<CellOutput>>,
  /// Execution counter for code cells.
  #[serde(default)]
  pub execution_count: Option<u64>,
}

/// A Jupyter notebook (nbformat 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Notebook {
  /// Sequential list of notebook cells.
  pub cells: Vec<Cell>,
  /// Top-level notebook metadata JSON.
  #[serde(default)]
  pub metadata: Value,
}

impl Notebook {
  /// Reads and parses a notebook from disk.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if reading or JSON deserialization fails.
  pub fn read_from(path: &Path) -> io::Result<Self> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(io::Error::other)
  }

  /// Extracts normalized text of all markdown cells into a starter prompt set.
  #[must_use]
  pub fn extract_starter_prompts(&self) -> HashSet<String> {
    let mut set = HashSet::new();
    for cell in &self.cells {
      if cell.cell_type == "markdown" {
        let text = cell.source.to_string_content();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
          set.insert(normalize_markdown(trimmed));
        }
      }
    }
    set
  }

  /// Sanitizes the notebook into clean Markdown ready for AI grading context.
  #[must_use]
  pub fn sanitize_to_markdown(
    &self,
    starter_prompts: Option<&HashSet<String>>,
  ) -> String {
    let mut out = String::new();

    for (idx, cell) in self.cells.iter().enumerate() {
      match cell.cell_type.as_str() {
        "markdown" => {
          let text = cell.source.to_string_content();
          let trimmed = text.trim();
          if trimmed.is_empty() {
            continue;
          }

          if let Some(prompts) = starter_prompts {
            let normalized = normalize_markdown(trimmed);
            if prompts.contains(&normalized) {
              out.push_str("<!-- [Instructions omitted] -->\n\n");
              continue;
            }
          }

          out.push_str(trimmed);
          out.push_str("\n\n");
        }
        "code" => {
          let code = cell.source.to_string_content();
          let trimmed_code = code.trim();
          if trimmed_code.is_empty() {
            continue;
          }

          let exec_str = cell
            .execution_count
            .map_or_else(|| " ".to_string(), |c| c.to_string());
          let _ = write!(out, "```python\n# [Cell {idx}] In [{exec_str}]:\n");
          out.push_str(trimmed_code);
          out.push_str("\n```\n");

          if let Some(outputs) = &cell.outputs {
            for output in outputs {
              if let Some(data) = &output.data {
                if data.contains_key("image/png")
                  || data.contains_key("image/jpeg")
                {
                  out.push_str("> [Plot Generated]\n\n");
                }
                if let Some(text_val) = data.get("text/plain") {
                  let text_str = match text_val {
                    Value::String(s) => s.clone(),
                    Value::Array(arr) => arr
                      .iter()
                      .filter_map(Value::as_str)
                      .collect::<Vec<_>>()
                      .join(""),
                    _ => String::new(),
                  };
                  format_output_stream(&mut out, &text_str);
                }
              }
              if let Some(stream_text) = &output.text {
                let text_str = stream_text.to_string_content();
                format_output_stream(&mut out, &text_str);
              }
              if let Some(ename) = &output.ename {
                let evalue = output.evalue.as_deref().unwrap_or("");
                let _ = write!(out, "> [Runtime Error: {ename}: {evalue}]\n\n");
              }
            }
          }
          out.push('\n');
        }
        _ => {}
      }
    }

    out
  }
}

/// Normalizes markdown text for set comparison by collapsing whitespace.
fn normalize_markdown(s: &str) -> String {
  s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strips ANSI terminal escape codes from a string.
#[must_use]
pub fn strip_ansi(s: &str) -> String {
  let mut result = String::with_capacity(s.len());
  let mut in_escape = false;
  for c in s.chars() {
    if c == '\x1b' {
      in_escape = true;
    } else if in_escape {
      if c.is_ascii_alphabetic() {
        in_escape = false;
      }
    } else {
      result.push(c);
    }
  }
  result
}

/// Formats and truncates output text blocks.
fn format_output_stream(out: &mut String, raw: &str) {
  let cleaned = strip_ansi(raw);
  let trimmed = cleaned.trim();
  if trimmed.is_empty() {
    return;
  }

  let lines: Vec<&str> = trimmed.lines().collect();
  out.push_str("```\n");
  if lines.len() <= 30 {
    out.push_str(trimmed);
  } else {
    for line in &lines[..15] {
      out.push_str(line);
      out.push('\n');
    }
    let _ = writeln!(out, "... [truncated {} lines] ...", lines.len() - 20);
    for line in &lines[lines.len() - 5..] {
      out.push_str(line);
      out.push('\n');
    }
  }
  out.push_str("\n```\n\n");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_strip_ansi() {
    let raw = "\x1b[32mHello\x1b[0m \x1b[1mWorld\x1b[0m";
    assert_eq!(
      strip_ansi(raw),
      "Hello World",
      "ANSI color codes should be stripped completely"
    );
  }

  #[test]
  fn test_notebook_deserialization_and_sanitization() {
    let json = r##"{
      "cells": [
        {
          "cell_type": "markdown",
          "source": ["# Problem 1\n", "Implement addition."]
        },
        {
          "cell_type": "code",
          "source": "def add(a, b):\n    return a + b",
          "execution_count": 1,
          "outputs": []
        }
      ],
      "metadata": {}
    }"##;

    let nb: Notebook =
      serde_json::from_str(json).expect("valid notebook JSON should parse");
    assert_eq!(nb.cells.len(), 2);

    let md = nb.sanitize_to_markdown(None);
    assert!(
      md.contains("# Problem 1"),
      "markdown prompt should be present when no starter prompts are given"
    );
    assert!(
      md.contains("def add(a, b):"),
      "code block should be preserved"
    );

    let starter = nb.extract_starter_prompts();
    let sanitized_stripped = nb.sanitize_to_markdown(Some(&starter));
    assert!(
      sanitized_stripped.contains("<!-- [Instructions omitted] -->"),
      "matching starter prompt markdown must be omitted"
    );
  }
}
