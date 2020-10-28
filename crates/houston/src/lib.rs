#![deny(missing_docs)]

//! Utilites for configuring the rover CLI tool.

mod error;
mod home;
mod profile;

pub use error::HoustonProblem;
pub use home::dir;

/// Utilites for saving, loading, and deleting configuration profiles.
pub use profile::LoadOpts;
pub use profile::Profile;

use std::fs;

/// Removes all configuration files from filesystem
pub fn clear() -> Result<(), HoustonProblem> {
    let profiles_dir = home::dir()?.join("profiles");
    tracing::debug!(profiles_dir = ?profiles_dir);
    let result = fs::remove_dir_all(profiles_dir);
    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            // we should not panic if a user tries to clear and has nothing to clear
            tracing::warn!("attemped to clear configuration but there was nothing to clear!");
            Ok(())
        }
    }
}
