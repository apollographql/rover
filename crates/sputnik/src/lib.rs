#![deny(missing_docs)]

//! Utilities for reporting anonymous usage data for the rover CLI tool.

mod error;
mod report;
mod session;

pub use error::SputnikError;
pub use report::Report;
pub use session::{Command, Session};
