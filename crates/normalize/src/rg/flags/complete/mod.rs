#![allow(warnings, clippy::all, unexpected_cfgs)]
// Vendored from ripgrep 14.1.1 (MIT/Unlicense)
/*!
Modules for generating completions for various shells.
*/

static ENCODINGS: &'static str = include_str!("encodings.sh");

pub(super) mod bash;
pub(super) mod fish;
pub(super) mod powershell;
pub(super) mod zsh;
