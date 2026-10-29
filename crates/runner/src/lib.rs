//! Automated Python test script runner and structured output parser.
//!
//! Owns discovering and executing `.bsg/assignments/<slug>/tests/*.py` against
//! student code using `uv` or virtual environments.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fs, io};

use serde::{Deserialize, Serialize};

/// One task or assertion result produced by a test script.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TestResultItem {
  /// Name or label of the test task.
  pub task: String,
  /// Whether the assertion or test script passed.
  pub passed: bool,
  /// Points awarded for this task.
  pub score: f64,
  /// Maximum points achievable for this task.
  pub max_score: f64,
  /// Human-readable explanation or error message.
  pub reasoning: String,
}

/// Aggregate report of all tests run for a submission.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct TestRunReport {
  /// Individual task results.
  pub items: Vec<TestResultItem>,
  /// True only if all executed test items passed.
  pub all_passed: bool,
  /// Sum of all awarded scores.
  pub total_score: f64,
}

/// Discovers all `.py` test scripts in an assignment's tests directory.
#[must_use]
pub fn find_test_scripts(tests_dir: &Path) -> Vec<PathBuf> {
  if !tests_dir.is_dir() {
    return Vec::new();
  }
  let mut scripts = Vec::new();
  if let Ok(entries) = fs::read_dir(tests_dir) {
    for entry in entries.flatten() {
      let p = entry.path();
      if p.is_file() && p.extension().is_some_and(|ext| ext == "py") {
        scripts.push(p);
      }
    }
  }
  scripts.sort();
  scripts
}

/// Runs a test script against a target student submission path.
///
/// # Errors
///
/// Returns [`io::Error`] if spawning the Python process fails.
pub fn run_test_script(
  script_path: &Path,
  student_target: &Path,
  venv_path: Option<&Path>,
) -> io::Result<Option<TestResultItem>> {
  let mut cmd = if let Some(venv) = venv_path {
    let python_bin = if cfg!(windows) {
      venv.join("Scripts").join("python.exe")
    } else {
      venv.join("bin").join("python")
    };
    Command::new(python_bin)
  } else {
    Command::new("uv")
  };

  if venv_path.is_none() {
    cmd.arg("run").arg("python");
  }

  let output = cmd
    .arg(script_path)
    .arg(student_target)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .map_err(|e| {
      if e.kind() == io::ErrorKind::NotFound {
        let tool = if venv_path.is_some() { "python" } else { "uv" };
        io::Error::new(
          io::ErrorKind::NotFound,
          format!(
            "'{tool}' was not found in PATH while running '{}'. Install \
             '{tool}' or configure an environment.",
            script_path.display()
          ),
        )
      } else {
        e
      }
    })?;

  let stdout = String::from_utf8_lossy(&output.stdout);

  // Attempt to parse structured JSON from stdout lines.
  for line in stdout.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with('{')
      && trimmed.ends_with('}')
      && let Ok(item) = serde_json::from_str::<TestResultItem>(trimmed)
    {
      return Ok(Some(item));
    }
  }

  // Fallback: evaluate raw process exit status.
  let task_name = script_path
    .file_stem()
    .map_or_else(|| "Test".to_string(), |s| s.to_string_lossy().to_string());

  if output.status.success() {
    Ok(Some(TestResultItem {
      task: task_name,
      passed: true,
      score: 1.0,
      max_score: 1.0,
      reasoning: "Test passed with zero exit code.".to_string(),
    }))
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let msg = if stderr.trim().is_empty() {
      stdout.trim().to_string()
    } else {
      stderr.trim().to_string()
    };
    Ok(Some(TestResultItem {
      task: task_name,
      passed: false,
      score: 0.0,
      max_score: 1.0,
      reasoning: format!("Execution failed: {msg}"),
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_test_result_item_deserialization() {
    let json = r#"{"task": "q1", "passed": true, "score": 5.0, "max_score": 5.0, "reasoning": "All checks passed"}"#;
    let item: TestResultItem =
      serde_json::from_str(json).expect("valid JSON test result should parse");
    assert_eq!(item.task, "q1");
    assert!(item.passed);
    assert_eq!(item.score, 5.0);
    assert_eq!(item.max_score, 5.0);
  }
}
