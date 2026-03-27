//! Shared sort infrastructure for session subcommands.
//!
//! Each command defines its own `SortField` enum. This module provides the
//! direction, key, and spec types that wrap any field type implementing
//! `DefaultDir + FromStr`.

use std::str::FromStr;

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Ascending,
    Descending,
}

/// Trait for sort field enums: provides the default direction when no +/- prefix is given.
pub trait DefaultDir: Sized + Copy {
    fn default_dir(self) -> SortDir;
}

/// A single sort key with direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey<F> {
    pub field: F,
    pub dir: SortDir,
}

/// Parsed `--sort` specification: a comma-separated list of sort keys, each optionally
/// prefixed with `-` (desc) or `+` (asc). When no prefix is given, the field's
/// `DefaultDir` implementation is used.
#[derive(Debug, Clone)]
pub struct SortSpec<F> {
    pub keys: Vec<SortKey<F>>,
}

impl<F> Default for SortSpec<F> {
    fn default() -> Self {
        SortSpec { keys: Vec::new() }
    }
}

impl<F: DefaultDir + FromStr<Err = String>> SortSpec<F> {
    /// Parse a `--sort` value string.
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut keys = Vec::new();
        for token in s.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let (dir, field_str) = if let Some(rest) = token.strip_prefix('-') {
                (Some(SortDir::Descending), rest)
            } else if let Some(rest) = token.strip_prefix('+') {
                (Some(SortDir::Ascending), rest)
            } else {
                (None, token)
            };
            let field = F::from_str(field_str)?;
            let dir = dir.unwrap_or_else(|| field.default_dir());
            keys.push(SortKey { field, dir });
        }
        Ok(SortSpec { keys })
    }
}
