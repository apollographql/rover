#![deny(missing_docs)]

//! Utilities for reporting anonymous usage data for the rover CLI tool.

mod error;
mod install_salt;
mod report;
mod session;

pub use error::SputnikError;
pub use install_salt::{GraphId, InstallSalt};
pub use report::Report;
pub use session::{Command, Environment, Outcome, Session};
