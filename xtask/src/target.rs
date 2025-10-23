use std::{collections::HashMap, env::consts, fmt};

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::ValueEnum;

use crate::utils::{PKG_PROJECT_NAME, PKG_PROJECT_ROOT};

const BREW_OPT: &[&str] = &["/usr/local/opt", "/opt/homebrew/Cellar"];

#[derive(ValueEnum, Debug, PartialEq, Clone)]
pub(crate) enum Target {
    #[value(name = "x86_64-unknown-linux-musl")]
    LinuxUnknownMusl,

    #[clap(name = "x86_64-unknown-linux-gnu")]
    LinuxUnknownGnu,

    #[clap(name = "aarch64-unknown-linux-gnu")]
    LinuxAarch64,

    #[clap(name = "x86_64-pc-windows-msvc")]
    WindowsMsvc,

    #[clap(name = "x86_64-apple-darwin")]
    MacOSAmd64,

    #[clap(name = "aarch64-apple-darwin")]
    MacOSAarch64,

    #[clap(skip)]
    Other,
}

impl Target {
    pub(crate) fn get_cargo_args(&self) -> Vec<String> {
        let mut target_args = vec![];

        if let Some(possible_value) = self.to_possible_value() {
            target_args.push("--target".to_string());
            target_args.push(possible_value.get_name().to_string());
        }

        if !self.composition_js() {
            target_args.push("--no-default-features".to_string());
        }
        target_args
    }

    pub(crate) fn is_other(&self) -> bool {
        Self::Other == *self
    }

    pub(crate) fn is_macos(&self) -> bool {
        Self::MacOSAmd64 == *self || Self::MacOSAarch64 == *self
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
                .map(|x| Utf8Path::new(x).join("openssl@3"))
                .find(|x| x.exists())
                .ok_or_else(|| {
                    anyhow!(
                        "OpenSSL v3 is not installed. Please install with `brew install \
                    openssl@3`"
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

    const fn composition_js(&self) -> bool {
        !matches!(self, Target::LinuxUnknownMusl)
    }

    pub(crate) fn get_bin_path(&self, release: bool) -> Utf8PathBuf {
        let mut target_dir = PKG_PROJECT_ROOT.join("target");
        if !self.is_other() {
            target_dir.push(self.to_string())
        }
        if release {
            target_dir.push("release")
        } else {
            target_dir.push("debug")
        };
        target_dir.join(format!("{}{}", PKG_PROJECT_NAME, consts::EXE_SUFFIX))
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

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.to_possible_value() {
            Some(possible_value) => possible_value.get_name().to_string(),
            None => "unknown-target".to_string(),
        };
        write!(f, "{msg}")
    }
}
