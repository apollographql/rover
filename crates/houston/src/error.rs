use std::io;

use rover_std::RoverStdError;
use thiserror::Error;

/// HoustonProblem is the type of Error that occured.
#[derive(Error, Debug)]
pub enum HoustonProblem {
    /// ConfigDirNotFound occurs when the default OS config can't be found.
    #[error("Could not determine default OS configuration directory.")]
    DefaultConfigDirNotFound,

    /// CouldNotCreateConfig occurs when a configuration directory could not be created.
    #[error("Could not create a configuration directory at '{0}'.")]
    CouldNotCreateConfigHome(String),

    /// InvalidOverrideConfigDir occurs when a user provides a path to a non-directory.
    #[error("'{0}' already exists and is not a directory.")]
    InvalidOverrideConfigDir(String),

    /// NoConfigFound occurs when a global configuration directory can't be found.
    #[error("Could not find a configuration directory at '{0}'.")]
    NoConfigFound(String),

    /// ProfileNotFound occurs when a profile with a specified name can't be found.
    #[error("There is no profile named '{0}'.")]
    ProfileNotFound(String),

    /// NoProfilesFound occurs when there are no profiles at all, often for new users
    #[error("No configuration profiles were found, and this command requires one.")]
    NoConfigProfiles,

    /// NoNonSensitiveConfigFound occurs when non-sensitive config can't be found for a profile.
    #[error("No non-sensitive configuration found for profile '{0}'.")]
    NoNonSensitiveConfigFound(String),

    /// CorruptedProfile occurs on Windows when `rover config auth` was run with older versions of Rover.
    #[error("The API key associated with profile '{0}' is corrupt.")]
    CorruptedProfile(String),

    /// PathNotUtf8 occurs when Houston encounters a file path that is not valid UTF-8
    #[error(transparent)]
    PathNotUtf8(#[from] camino::FromPathBufError),

    /// TomlSerialization occurs when a profile's configuration can't be serialized to a String.
    #[error(transparent)]
    TomlSerialization(#[from] toml::ser::Error),

    /// TomlDeserialization occurs when a profile's configuration can't be deserialized from a String.
    #[error(transparent)]
    TomlDeserialization(#[from] toml::de::Error),

    /// io::Error occurs when any given std::io::Error arises.
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// AdhocError comes from anyhow::Error
    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),

    /// RoverStdError comes from RoverStdError
    #[error(transparent)]
    RoverStdError(#[from] RoverStdError),
}
