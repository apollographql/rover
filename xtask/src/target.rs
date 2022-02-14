use anyhow::anyhow;
use camino::Utf8Path;

use std::{collections::HashMap, fmt, str::FromStr};

use crate::Result;

pub(crate) const TARGET_MUSL_LINUX: &str = "x86_64-unknown-linux-musl";
pub(crate) const TARGET_GNU_LINUX: &str = "x86_64-unknown-linux-gnu";
pub(crate) const TARGET_WINDOWS: &str = "x86_64-pc-windows-msvc";
pub(crate) const TARGET_X86_64_MACOS: &str = "x86_64-apple-darwin";
pub(crate) const TARGET_ARM_MACOS: &str = "aarch64-apple-darwin";
pub(crate) const TARGET_UNIVERSAL_MACOS: &str = "unknown-apple-darwin";
const BREW_OPT: &[&str] = &["/usr/local/opt", "/opt/homebrew/Cellar"];

pub(crate) const POSSIBLE_TARGETS: [&str; 6] = [
    TARGET_MUSL_LINUX,
    TARGET_GNU_LINUX,
    TARGET_WINDOWS,
    TARGET_UNIVERSAL_MACOS,
    TARGET_X86_64_MACOS,
    TARGET_ARM_MACOS,
];

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Target {
    MuslLinux,
    GnuLinux,
    Windows,
    ArmMacOS,
    NotArmMacOS,
    UniversalMacOS,
    Other,
}

impl Target {
    /// This function will return all of the options needed
    /// to build for a specific target. It is a Vec<Vec<String>>
    /// because sometimes you need more than one cargo build
    /// to be run for an architecture (namely universal MacOS binaries)
    pub(crate) fn get_all_cargo_args(&self) -> Vec<Vec<String>> {
        let mut cargo_args = vec![];
        for cargo_target in self.cargo_targets() {
            let mut current_args = vec![];
    
            if !self.is_other() {
                current_args.push("--target".to_string());
                current_args.push(cargo_target.to_string());
            }
            if !self.composition_js() {
                current_args.push("--no-default-features".to_string());
            }
            cargo_args.push(current_args);
        }
        cargo_args
    }

    pub(crate) fn is_other(&self) -> bool {
        Self::Other == *self
    }

    pub(crate) fn get_env(&self) -> Result<HashMap<String, String>> {
        let mut env = HashMap::new();
        match self {
            Target::GnuLinux | Target::MuslLinux => {
                env.insert("OPENSSL_STATIC".to_string(), "1".to_string());
            }
            Target::UniversalMacOS | Target::ArmMacOS | Target::NotArmMacOS => {
                let openssl_path = BREW_OPT
                    .iter()
                    .map(|x| Utf8Path::new(x).join("openssl@1.1"))
                    .find(|x| x.exists())
                    .ok_or_else(|| {
                        anyhow!(
                            "OpenSSL v1.1 is not installed. Please install with `brew install \
                        openssl@1.1`"
                        )
                    })?;

                env.insert("OPENSSL_ROOT_DIR".to_string(), openssl_path.to_string());
                env.insert("OPENSSL_STATIC".to_string(), "1".to_string());
            }
            Target::Windows => {
                env.insert(
                    "RUSTFLAGS".to_string(),
                    "-Ctarget-feature=+crt-static".to_string(),
                );
            },
            _ => {}
        };
        Ok(env)
    }

    fn cargo_targets(&self) -> Vec<String> {
        match &self {
            Self::UniversalMacOS => vec![TARGET_X86_64_MACOS.to_string(), TARGET_ARM_MACOS.to_string()],
            Self::ArmMacOS => vec![TARGET_ARM_MACOS.to_string()],
            Self::NotArmMacOS => vec![TARGET_X86_64_MACOS.to_string()],
            Self::Windows => vec![TARGET_WINDOWS.to_string()],
            Self::GnuLinux => vec![TARGET_GNU_LINUX.to_string()],
            Self::MuslLinux => vec![TARGET_MUSL_LINUX.to_string()],
            Self::Other => vec![]
        }
    }

    fn composition_js(&self) -> bool {
        !matches!(self, Target::MuslLinux)
    }
}

impl Default for Target {
    fn default() -> Self {
        let mut target = Target::Other;

        if cfg!(target_os = "macos") {
            if cfg!(target_arch = "x86_64") {
                target = Target::NotArmMacOS
            } else {
                target = Target::ArmMacOS
            }
        } else if cfg!(target_os = "windows") {
            target = Target::Windows
        } else if cfg!(target_os = "linux") {
            if cfg!(target_arch = "x86_64") {
                if cfg!(target_env = "gnu") {
                    target = Target::GnuLinux
                } else if cfg!(target_env = "musl") {
                    target = Target::MuslLinux
                }
            }
        }

        target
    }
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            TARGET_MUSL_LINUX => Ok(Self::MuslLinux),
            TARGET_GNU_LINUX => Ok(Self::GnuLinux),
            TARGET_WINDOWS => Ok(Self::Windows),
            TARGET_UNIVERSAL_MACOS => Ok(Self::UniversalMacOS),
            TARGET_ARM_MACOS => Ok(Self::ArmMacOS),
            TARGET_X86_64_MACOS => Ok(Self::NotArmMacOS),
            _ => Ok(Self::Other),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match &self {
            Target::MuslLinux => TARGET_MUSL_LINUX,
            Target::GnuLinux => TARGET_GNU_LINUX,
            Target::Windows => TARGET_WINDOWS,
            Target::UniversalMacOS => TARGET_UNIVERSAL_MACOS,
            Target::ArmMacOS => TARGET_ARM_MACOS,
            Target::NotArmMacOS => TARGET_X86_64_MACOS,
            Target::Other => "unknown-target",
        };
        write!(f, "{}", msg)
    }
}
