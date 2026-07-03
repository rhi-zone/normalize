//! Path utilities for normalize data directories.
//!
//! The primitive lives in `normalize-facts` (the lowest crate that resolves the
//! index path — see `normalize_facts::paths`) and is re-exported through
//! `normalize-index`. Re-exported here so existing `crate::paths::get_normalize_dir`
//! call sites are unchanged.

pub use normalize_index::get_normalize_dir;
