---
name: bsg-grading
description: Guide for grading Brightspace assignments using the bsg CLI and AI evaluation.
---

# Brightspace Grader (`bsg`) Skill

A workflow guide for AI agents and course staff grading Brightspace assignments
using `bsg`.

## Core Commands

- `bsg ingest`: Ingests bulk ZIP downloads from `archives/` into assignment folders.
- `bsg status <ASSIGNMENT> [-g GROUP] [-r RANGE]`: Views submissions, late passes, and delay.
  - `bsg status excuse <ASSIGNMENT> <STUDENT> [--reason "..."]`: Marks student excused.
- `bsg grade <ASSIGNMENT> [STUDENT] [-g GROUP] [-r RANGE]`:
  - `--check-only`: Runs test scripts in `.bsg/assignments/<slug>/tests/*.py`.
  - `--prep-only`: Generates clean markdown (`.grade.md`) with base64 images stripped and instructions omitted.
- `bsg repo`: Manages workspace initialization (`init`), sync (`sync`), and publishing (`publish`).

## Grading Workflow for AI Agents

1. **Verify Slices**:
   Check assigned TA cohort using `bsg status <hw> -g <group>` (e.g. `hw3 -g G1` or `hw4 -g G2`).
2. **Prepare Code**:
   Inspect the submission notebook or script.
   Run `bsg grade <hw> <student> --prep-only` to generate a stripped, context-optimized `.grade.md`.
3. **Evaluate Against Rubric**:
   Read `.bsg/assignments/<hw>/rubric.toml`.
   Follow the grading directive:
   - **Strict on Rubric**: Check every task and subtask criterion.
   - **Constructive Leniency**: Reward valid logic, correct approach, and honest effort despite minor syntax, import, or typing errors.
   - **Late Passes**: If student has late passes remaining ($\le 4$), late submissions consume penalty-free passes (0% penalty).
4. **Report Scores**:
   Present score breakdown per rubric task with concise, actionable student feedback.
