<!-- markdownlint-disable MD033 MD041 -->
<div align="center">

# `bsg`

**A CLI toolkit for grading Brightspace assignments.**

[![Release](https://img.shields.io/github/v/release/arvinduh/grader?style=flat-square&color=1e293b)](https://github.com/arvinduh/grader/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-1e293b?style=flat-square)](#installation)

</div>
<!-- markdownlint-enable MD033 -->

---

`bsg` streamlines grading workflows for Brightspace courses. It starts by taking
raw, chaotic bulk-download archives and turning them into clean, predictable
student files ready for grading.

## Installation

### macOS & Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/arvinduh/grader/releases/latest/download/grader-installer.sh | sh
```

### Windows (PowerShell)

```powershell
powershell -c "irm https://github.com/arvinduh/grader/releases/latest/download/grader-installer.ps1 | iex"
```

### build from source

```bash
cargo install --git https://github.com/arvinduh/grader
```

---

## Ingesting Downloads

Brightspace bulk exports are full of nested zips, student ID prefixes, duplicate
upload tags (`(1)`), and multiple submission attempts.

`bsg ingest` cleans and organizes everything in one pass:

```txt
📦 "HW1 Download Oct 14, 2026.zip"
├── 📁 109283 - Doe, John - Sep 24.../ hw1.ipynb
├── 📁 109283 - Doe, John - Sep 25.../ hw1 (1).ipynb   # Keeps latest attempt
└── 📁 847291 - Smith, Jane.../ submission.zip        # Unpacks student zips
```

⬇️

```txt
📂 HW1/
├── 📄 doe_john.ipynb      # Single file flattened to <slug>.<ext>
└── 📁 smith_jane/         # Multi-file submissions cleanly unpacked
    ├── mod.py
    └── main.py
```

- **Deduplicates attempts** — picks each student's newest submission and strips
  browser copy numbers.
- **Unpacks nested zips** — extracts student archives automatically.
- **Normalizes names** — standardizes files to `last_first` and drops OS junk
  (`.DS_Store`, `__MACOSX`).
- **Detects duplicates & conflicts** — warns and skips ambiguous input archives
  sharing an assignment name; checks existing manifests to avoid accidental
  overwrites.

```bash
# Ingest beside the archive
bsg ingest "HW1 Download.zip"

# Ingest all downloads into a target directory
bsg ingest ~/Downloads/*.zip -o ./grading/

# Force overwrite and re-extract an existing destination
bsg ingest "HW1 Download.zip" -f

# Increase logging verbosity (-v for WARN, -vv for INFO, -vvv for DEBUG)
bsg -v ingest "HW1 Download.zip"
```

---

## Grading & Workflow Commands

- **`bsg status <ASSIGNMENT>`**: View student submission history, delay, and
  remaining late pass balance.

  ```bash
  bsg status hw1 -g G1          # Filter by assigned TA group
  bsg status hw1 -r Adebayo..Luu # Filter by last-name range
  ```

- **`bsg grade <ASSIGNMENT> [STUDENT]`**: Run automated Python assertions and
  prepare clean markdown for AI evaluation.

  ```bash
  bsg grade hw1 --check-only    # Only run test scripts (*.py)
  bsg grade hw1 --prep-only     # Strip boilerplate and generate .grade.md
  ```

- **`bsg repo`**: Initialize and synchronize private course authoring assets.

  ```bash
  bsg repo init                 # Create local .bsg workspace
  bsg repo sync                 # Pull latest rubrics, tests, and solutions
  ```

---

## Workspace Structure

The project is structured as a fine-grained Cargo workspace adhering to the
Google Rust Style Guide:

- `crates/bsg`: Command-line interface and terminal presentation.
- `crates/ingest`: Brightspace archive scanning, extraction, and manifest
  engine.
- `crates/notebook`: Jupyter notebook parsing and AI grading context
  preparation.
- `crates/runner`: Automated Python test script runner and assertion parser.
- `crates/ledger`: Late pass policy tracking and penalty calculation engine.
- `crates/workspace`: Course repository discovery and configuration models.

---

## Updating

Update the binary in place directly from GitHub Releases:

```bash
bsg update          # Upgrade to latest
bsg update --check  # Check for updates without installing
```
