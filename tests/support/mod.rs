//! Test support shared across test binaries.
//!
//! `tests/main.rs` includes this today; `tests/test_e2e.rs` picks it up with
//! the end-to-end plugin coverage. Each binary compiles the tree separately
//! and uses a different subset of it, so unused items here are expected rather
//! than a sign of dead code.
#![allow(dead_code)]

pub mod network;
pub mod plugin_levels;
