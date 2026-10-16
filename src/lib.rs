//! Ingests Brightspace bulk assignment downloads for grading.
//!
//! `ingest` owns the whole pipeline, from finding downloads to writing
//! per-student files and the manifest later commands read.

pub mod ingest;

pub use ingest::Error;
