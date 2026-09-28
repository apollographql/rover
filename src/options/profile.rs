use std::fmt::Display;

use serde::{Deserialize, Serialize};

pub const DEFAULT_PROFILE: &str = "default";

/// The profile a command resolved to. Built once, from `Rover`'s global
/// `--profile` flag, and passed down to whichever command needs it - this is
/// no longer a `clap` arg in its own right (see `Rover::profile_name`).
#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOpt {
    #[serde(skip_serializing)]
    pub profile_name: String,

    /// Whether `--profile` was passed explicitly, or this is the implicit
    /// default profile (`--profile` omitted). See [`ProfileSelection`].
    #[serde(skip_serializing)]
    pub selection: ProfileSelection,
}

/// Whether `--profile` was passed explicitly on the command line - including
/// `--profile default` typed literally - or the invocation fell through to
/// the implicit default profile because `--profile` was omitted.
///
/// Distinguishing the two (rather than collapsing both into "the profile
/// named `default`") is what lets `--profile default` behave differently
/// from omitting the flag: it's the `explicit_profile`/`default_profile`
/// distinction `rover config show` reports, and it's the condition the
/// override notice (an environment variable overriding an *explicitly
/// selected* profile) checks.
#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSelection {
    /// `--profile <name>` was passed, including `--profile default` typed
    /// literally.
    Explicit,
    /// `--profile` was omitted; `profile_name` is [`DEFAULT_PROFILE`].
    #[cfg_attr(test, default)]
    Default,
}

impl Display for ProfileOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.profile_name)
    }
}
