//! Ratchet service module — re-exports from normalize-ratchet.
//!
//! The implementation lives in the `normalize-ratchet` crate.
//! This module exists only to satisfy the service layer pattern where
//! each subcommand has a corresponding module under `service/`.

pub use normalize_ratchet::service::RatchetService;
