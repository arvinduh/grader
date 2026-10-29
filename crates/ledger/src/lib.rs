//! Live student late pass ledger and penalty calculation engine.
//!
//! Owns `.bsg/ledger.json` and evaluates delay, pass deductions, and penalties
//! strictly according to course policy.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use bsg_workspace::LatePolicyConfig;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Exemption status for a student's submission.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[non_exhaustive]
pub enum ExcusedStatus {
  /// Normal submission subject to late pass consumption and penalties.
  #[default]
  Active,
  /// Excused submission with zero pass deductions and zero penalty.
  Excused,
}

impl From<bool> for ExcusedStatus {
  fn from(excused: bool) -> Self {
    if excused { Self::Excused } else { Self::Active }
  }
}

/// Record of one assignment's late pass consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassUsageRecord {
  /// Assignment slug or name.
  pub assignment: String,
  /// Submission timestamp as recorded by Brightspace.
  pub submitted_at: NaiveDateTime,
  /// Effective due date timestamp used for evaluation.
  pub due_date: NaiveDateTime,
  /// Calculated delay in hours.
  pub delay_hours: f64,
  /// Number of late passes consumed for this submission.
  pub passes_consumed: u32,
  /// Fractional score penalty (0.0 to 1.0) applied after pass exhaustion.
  pub penalty_percent: f64,
}

/// One student's cumulative pass status and submission history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StudentLedgerEntry {
  /// Total passes consumed over the semester.
  pub passes_used: u32,
  /// Remaining passes available for future assignments.
  pub passes_remaining: u32,
  /// Chronological log of submission evaluations.
  pub history: Vec<PassUsageRecord>,
}

/// The semester-wide late pass ledger stored at `.bsg/ledger.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ledger {
  /// Active course policy governing penalties and pass durations.
  pub policy: LatePolicyConfig,
  /// Student ledger entries keyed by student slug.
  pub students: BTreeMap<String, StudentLedgerEntry>,
}

/// The result of evaluating a submission timestamp against policy.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EvaluationResult {
  /// Total hours late past the due date and grace period.
  pub delay_hours: f64,
  /// Passes automatically deducted.
  pub passes_consumed: u32,
  /// Fractional score deduction (0.0 to 1.0).
  pub penalty_percent: f64,
}

impl Ledger {
  /// Creates a new ledger initialized with a given policy.
  #[must_use]
  pub fn new(policy: LatePolicyConfig) -> Self {
    Self {
      policy,
      students: BTreeMap::new(),
    }
  }

  /// Reads the ledger from disk or creates a default one if it does not exist.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if reading or parsing an existing file fails.
  pub fn read_or_default(
    path: &Path,
    policy: &LatePolicyConfig,
  ) -> io::Result<Self> {
    if !path.is_file() {
      return Ok(Self::new(policy.clone()));
    }
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(io::Error::other)
  }

  /// Writes the ledger to disk.
  ///
  /// # Errors
  ///
  /// Returns [`io::Error`] if writing fails.
  pub fn write_to(&self, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
    fs::write(path, json)
  }

  /// Evaluates submission timing against due date, updating the student's
  /// record.
  pub fn record_submission(
    &mut self,
    slug: &str,
    assignment: &str,
    submitted_at: NaiveDateTime,
    due_date: NaiveDateTime,
    status: ExcusedStatus,
    extension: Option<NaiveDateTime>,
  ) -> EvaluationResult {
    let effective_due = extension.unwrap_or(due_date);
    let entry = self.students.entry(slug.to_string()).or_insert_with(|| {
      StudentLedgerEntry {
        passes_used: 0,
        passes_remaining: self.policy.initial_passes,
        history: Vec::new(),
      }
    });

    if status == ExcusedStatus::Excused {
      let res = EvaluationResult {
        delay_hours: 0.0,
        passes_consumed: 0,
        penalty_percent: 0.0,
      };
      entry.history.push(PassUsageRecord {
        assignment: assignment.to_string(),
        submitted_at,
        due_date: effective_due,
        delay_hours: 0.0,
        passes_consumed: 0,
        penalty_percent: 0.0,
      });
      return res;
    }

    let diff = submitted_at.signed_duration_since(effective_due);
    let total_mins = diff.num_minutes();

    if total_mins <= i64::from(self.policy.grace_period_mins) {
      let res = EvaluationResult {
        delay_hours: (total_mins.max(0) as f64) / 60.0,
        passes_consumed: 0,
        penalty_percent: 0.0,
      };
      entry.history.push(PassUsageRecord {
        assignment: assignment.to_string(),
        submitted_at,
        due_date: effective_due,
        delay_hours: res.delay_hours,
        passes_consumed: 0,
        penalty_percent: 0.0,
      });
      return res;
    }

    let delay_hours = (total_mins as f64) / 60.0;
    let period_hours = (f64::from(self.policy.pass_duration_days)) * 24.0;
    let period_count = (delay_hours / period_hours).ceil() as u32;

    let passes_to_use = period_count.min(entry.passes_remaining);
    let uncovered = period_count.saturating_sub(passes_to_use);
    let penalty = ((f64::from(uncovered)) * self.policy.penalty_per_period)
      .min(self.policy.max_penalty);

    entry.passes_remaining =
      entry.passes_remaining.saturating_sub(passes_to_use);
    entry.passes_used += passes_to_use;

    let res = EvaluationResult {
      delay_hours,
      passes_consumed: passes_to_use,
      penalty_percent: penalty,
    };

    entry.history.push(PassUsageRecord {
      assignment: assignment.to_string(),
      submitted_at,
      due_date: effective_due,
      delay_hours,
      passes_consumed: passes_to_use,
      penalty_percent: penalty,
    });

    res
  }
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use super::*;

  #[test]
  fn test_on_time_submission_within_grace_period() {
    let policy = LatePolicyConfig::default();
    let mut ledger = Ledger::new(policy);

    let due = NaiveDate::from_ymd_opt(2026, 3, 1)
      .expect("valid date")
      .and_hms_opt(23, 59, 0)
      .expect("valid time");
    let sub = NaiveDate::from_ymd_opt(2026, 3, 2)
      .expect("valid date")
      .and_hms_opt(0, 10, 0)
      .expect("valid time"); // 11 mins late, within 15 min grace

    let res = ledger.record_submission(
      "student_a",
      "hw1",
      sub,
      due,
      ExcusedStatus::Active,
      None,
    );
    assert_eq!(res.passes_consumed, 0);
    assert_eq!(res.penalty_percent, 0.0);
    assert_eq!(ledger.students["student_a"].passes_remaining, 4);
  }

  #[test]
  fn test_late_pass_consumption_and_penalty() {
    let policy = LatePolicyConfig::default();
    let mut ledger = Ledger::new(policy);

    let due = NaiveDate::from_ymd_opt(2026, 3, 1)
      .expect("valid date")
      .and_hms_opt(23, 59, 0)
      .expect("valid time");
    // 4 days late -> needs 2 passes (each pass is 3 days)
    let sub = NaiveDate::from_ymd_opt(2026, 3, 5)
      .expect("valid date")
      .and_hms_opt(23, 59, 0)
      .expect("valid time");

    let res = ledger.record_submission(
      "student_b",
      "hw1",
      sub,
      due,
      ExcusedStatus::Active,
      None,
    );
    assert_eq!(res.passes_consumed, 2);
    assert_eq!(res.penalty_percent, 0.0);
    assert_eq!(ledger.students["student_b"].passes_remaining, 2);
  }
}
