use anyhow::anyhow;

use std::{fmt, str::FromStr};

const TARGET_MUSL_LINUX: &str = "x86_64-unknown-linux-musl";
const TARGET_GNU_LINUX: &str = "x86_64-unknown-linux-gnu";
const TARGET_WINDOWS: &str = "x86_64-pc-windows-msvc";
const TARGET_MACOS: &str = "x86_64-apple-darwin";

pub(crate) const POSSIBLE_TARGETS: [&str; 4] = [
    TARGET_MUSL_LINUX,
    TARGET_GNU_LINUX,
    TARGET_WINDOWS,
    TARGET_MACOS,
];

#[derive(Debug, Clone)]
pub(crate) enum Target {
    MuslLinux,
    GnuLinux,
    Windows,
    MacOS,
}

impl Target {
    pub(crate) fn composition_js(&self) -> bool {
        !matches!(self, Target::MuslLinux)
    }
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            TARGET_MUSL_LINUX => Ok(Self::MuslLinux),
            TARGET_GNU_LINUX => Ok(Self::GnuLinux),
            TARGET_WINDOWS => Ok(Self::Windows),
            TARGET_MACOS => Ok(Self::MacOS),
            unknown_target => Err(anyhow!(
                "`{}` is not a supported compilation target.",
                unknown_target
            )),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match &self {
            Target::MuslLinux => TARGET_MUSL_LINUX,
            Target::GnuLinux => TARGET_GNU_LINUX,
            Target::Windows => TARGET_WINDOWS,
            Target::MacOS => TARGET_MACOS,
        };
        write!(f, "{}", msg)
    }
}
