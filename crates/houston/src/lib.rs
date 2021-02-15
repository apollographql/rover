#![deny(missing_docs)]

//! Utilites for configuring the rover CLI tool.

mod config;
mod error;
mod profile;

pub use config::Config;
pub use error::HoustonProblem;

pub use profile::mask_key;
/// Utilites for saving, loading, and deleting configuration profiles.
pub use profile::LoadOpts;
pub use profile::Profile;
