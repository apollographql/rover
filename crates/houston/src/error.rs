use thiserror::Error;

use std::io;

/// HoustonProblem is the type of Error that occured.
#[derive(Error, Debug)]
pub enum HoustonProblem {
    /// ConfigDirNotFound occurs when the default OS config can't be found.
    #[error("Could not determine default OS config directory.")]
    ConfigDirNotFound,

    /// NoConfigFound occurs when a global configuration directory can't be found.
    #[error("Could not find a global configuration directory.")]
    NoConfigFound,

    /// ProfileNotFound occurs when a profile with a specified name can't be found.
    #[error("There is no profile named \"{0}\"")]
    ProfileNotFound(String),

    /// NoNonSensitiveConfigFound occurs when non-sensitive config can't be found for a profile.
    #[error("No non-sensitive config found for profile \"{0}\"")]
    NoNonSensitiveConfigFound(String),

    /// TomlSerialization occurs when a profile's configuration can't be serialized to a String.
    #[error(transparent)]
    TomlSerialization(#[from] toml::ser::Error),

    /// TomlDeserialization occurs when a profile's configruation can't be deserialized from a String.
    #[error(transparent)]
    TomlDeserialization(#[from] toml::de::Error),

    /// IOError occurs when any given std::io::Error arises.
    #[error(transparent)]
    IOError(#[from] io::Error),
}
