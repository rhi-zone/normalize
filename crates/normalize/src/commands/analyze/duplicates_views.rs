//! Unified duplicates command — groups duplicate-functions, duplicate-blocks,
//! similar-functions, and similar-blocks views behind `--scope` and `--similar` flags.

use crate::commands::analyze::duplicates::{
    DuplicateBlocksReport, DuplicateFunctionsReport, SimilarBlocksReport, SimilarFunctionsReport,
};
use normalize_output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Scope of duplicate detection: function-level or block-level.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateScope {
    #[default]
    Functions,
    Blocks,
}

impl FromStr for DuplicateScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "functions" | "function" => Ok(DuplicateScope::Functions),
            "blocks" | "block" => Ok(DuplicateScope::Blocks),
            _ => Err(format!(
                "invalid scope '{}': expected 'functions' or 'blocks'",
                s
            )),
        }
    }
}

impl fmt::Display for DuplicateScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DuplicateScope::Functions => write!(f, "functions"),
            DuplicateScope::Blocks => write!(f, "blocks"),
        }
    }
}

/// Unified duplicates output — one of the four detection modes.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "view")]
pub enum DuplicatesOutput {
    /// Exact duplicate functions
    #[serde(rename = "duplicate-functions")]
    DuplicateFunctions(DuplicateFunctionsReport),
    /// Exact duplicate blocks
    #[serde(rename = "duplicate-blocks")]
    DuplicateBlocks(DuplicateBlocksReport),
    /// Similar functions (MinHash LSH)
    #[serde(rename = "similar-functions")]
    SimilarFunctions(SimilarFunctionsReport),
    /// Similar blocks (MinHash LSH)
    #[serde(rename = "similar-blocks")]
    SimilarBlocks(SimilarBlocksReport),
}

impl OutputFormatter for DuplicatesOutput {
    fn format_text(&self) -> String {
        match self {
            DuplicatesOutput::DuplicateFunctions(r) => r.format_text(),
            DuplicatesOutput::DuplicateBlocks(r) => r.format_text(),
            DuplicatesOutput::SimilarFunctions(r) => r.format_text(),
            DuplicatesOutput::SimilarBlocks(r) => r.format_text(),
        }
    }

    fn format_pretty(&self) -> String {
        match self {
            DuplicatesOutput::DuplicateFunctions(r) => r.format_pretty(),
            DuplicatesOutput::DuplicateBlocks(r) => r.format_pretty(),
            DuplicatesOutput::SimilarFunctions(r) => r.format_pretty(),
            DuplicatesOutput::SimilarBlocks(r) => r.format_pretty(),
        }
    }
}
