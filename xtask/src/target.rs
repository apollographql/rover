use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};

use std::{collections::HashMap, env::consts, fmt, str::FromStr};

use crate::{
    utils::{PKG_PROJECT_NAME, PKG_PROJECT_ROOT},
    Result,
};

pub(crate) const TARGET_LINUX_UNKNOWN_MUSL: &str = "x86_64-unknown-linux-musl";
pub(crate) const TARGET_LINUX_UNKNOWN_GNU: &str = "x86_64-unknown-linux-gnu";
pub(crate) const TARGET_LINUX_ARM: &str = "aarch64-unknown-linux-gnu";
pub(crate) const TARGET_WINDOWS_MSVC: &str = "x86_64-pc-windows-msvc";
pub(crate) const TARGET_MACOS_AMD64: &str = "x86_64-apple-darwin";
pub(crate) const TARGET_MACOS_ARM: &str = "aarch64-apple-darwin";
pub(crate) const TARGET_MACOS_UNIVERSAL: &str = "unknown-apple-darwin";
const BREW_OPT: &[&str] = &["/usr/local/opt", "/opt/homebrew/Cellar"];

pub(crate) const POSSIBLE_TARGETS: [&str; 6] = [
    TARGET_LINUX_UNKNOWN_MUSL,
    TARGET_LINUX_UNKNOWN_GNU,
    TARGET_LINUX_ARM,
    TARGET_WINDOWS_MSVC,
    TARGET_MACOS_AMD64,
    TARGET_MACOS_ARM,
];

pub(crate) const DOCKER_TARGETS: [&str; 1] = [TARGET_LINUX_UNKNOWN_GNU];

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Target {
    LinuxUnknownMusl,
    LinuxUnknownGnu,
    LinuxAarch64,
    WindowsMsvc,
    MacOSAmd64,
    MacOSAarch64,
    MacOSUniversal,
    Other,
}

impl Target {
    /// This function will return all of the options needed
    /// to build for a specific target. It is a Vec<Vec<String>>
    /// because sometimes you need more than one cargo build
    /// to be run for an architecture (namely universal MacOS binaries)
    pub(crate) fn get_all_cargo_args(&self) -> Vec<Vec<String>> {
        let mut all_cargo_args = Vec::new();
        for cargo_target in self.get_cargo_targets() {
            let mut target_args = Vec::new();
            if !self.is_other() {
                target_args.push("--target".to_string());
                target_args.push(cargo_target.to_string());
            }
            if !self.composition_js() {
                target_args.push("--no-default-features".to_string());
            }
            all_cargo_args.push(target_args);
        }
        all_cargo_args
    }

    /// This function returns a Vec of values for cargo's --target flag
    /// If it returns multiple, multiple targets should be built as a part of this target
    pub(crate) fn get_cargo_targets(&self) -> Vec<Target> {
        match &self {
            Self::MacOSUniversal => {
                vec![Self::MacOSAarch64, Self::MacOSAmd64]
            }
            _ => vec![self.clone()],
        }
    }

    pub(crate) fn is_other(&self) -> bool {
        Self::Other == *self
    }

    pub(crate) fn is_macos(&self) -> bool {
        Self::MacOSAmd64 == *self || Self::MacOSAarch64 == *self || Self::MacOSUniversal == *self
    }

    pub(crate) fn is_linux(&self) -> bool {
        Self::LinuxAarch64 == *self
            || Self::LinuxUnknownGnu == *self
            || Self::LinuxUnknownMusl == *self
    }

    pub(crate) fn is_windows(&self) -> bool {
        Self::WindowsMsvc == *self
    }

    pub(crate) fn get_env(&self) -> Result<HashMap<String, String>> {
        let mut env = HashMap::new();
        if self.is_linux() {
            env.insert("OPENSSL_STATIC".to_string(), "1".to_string());
        } else if self.is_macos() {
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
        } else if self.is_windows() {
            env.insert(
                "RUSTFLAGS".to_string(),
                "-Ctarget-feature=+crt-static".to_string(),
            );
        }
        Ok(env)
    }

    fn composition_js(&self) -> bool {
        !matches!(self, Target::LinuxUnknownMusl)
    }

    pub(crate) fn get_bin_paths(&self, release: bool) -> Vec<Utf8PathBuf> {
        let mut bin_paths = Vec::new();
        let base = PKG_PROJECT_ROOT.join("target");
        for target in self.get_cargo_targets() {
            let target_dir = if self.is_other() {
                base.clone()
            } else {
                base.join(target.to_string())
            };
            let mode_dir = if release {
                target_dir.join("release")
            } else {
                target_dir.join("debug")
            };
            let bin_path = mode_dir.join(format!("{}{}", PKG_PROJECT_NAME, consts::EXE_SUFFIX));
            bin_paths.push(bin_path)
        }

        bin_paths
    }
}

impl Default for Target {
    fn default() -> Self {
        let mut result = Target::Other;
        if cfg!(target_os = "windows") {
            if cfg!(target_arch = "x86_64") {
                result = Target::WindowsMsvc;
            }
        } else if cfg!(target_os = "linux") {
            if cfg!(target_env = "musl") {
                result = Target::LinuxUnknownMusl
            } else if cfg!(target_env = "gnu") {
                if cfg!(target_arch = "x86_64") {
                    result = Target::LinuxUnknownGnu
                } else if cfg!(target_arch = "aarch64") {
                    result = Target::LinuxAarch64
                }
            }
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "x86_64") {
                result = Target::MacOSAmd64
            } else if cfg!(target_arch = "aarch64") {
                result = Target::MacOSAarch64
            }
        }
        result
    }
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            TARGET_LINUX_UNKNOWN_MUSL => Ok(Self::LinuxUnknownMusl),
            TARGET_LINUX_UNKNOWN_GNU => Ok(Self::LinuxUnknownGnu),
            TARGET_LINUX_ARM => Ok(Self::LinuxAarch64),
            TARGET_WINDOWS_MSVC => Ok(Self::WindowsMsvc),
            TARGET_MACOS_AMD64 => Ok(Self::MacOSAmd64),
            TARGET_MACOS_ARM => Ok(Self::MacOSAarch64),
            _ => Ok(Self::Other),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match &self {
            Target::LinuxUnknownMusl => TARGET_LINUX_UNKNOWN_MUSL,
            Target::LinuxUnknownGnu => TARGET_LINUX_UNKNOWN_GNU,
            Target::LinuxAarch64 => TARGET_LINUX_ARM,
            Target::WindowsMsvc => TARGET_WINDOWS_MSVC,
            Target::MacOSAmd64 => TARGET_MACOS_AMD64,
            Target::MacOSAarch64 => TARGET_MACOS_ARM,
            Target::MacOSUniversal => TARGET_MACOS_UNIVERSAL,
            Target::Other => "unknown-target",
        };
        write!(f, "{}", msg)
    }
}
