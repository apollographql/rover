use anyhow::{anyhow, Result};
use camino::Utf8Path;
use clap::ValueEnum;

use std::{collections::HashMap, fmt};

const BREW_OPT: &[&str] = &["/usr/local/opt", "/opt/homebrew/Cellar"];

#[derive(ValueEnum, Debug, PartialEq, Clone)]
pub(crate) enum Target {
    #[value(name = "x86_64-unknown-linux-musl")]
    MuslLinux,

    #[clap(name = "x86_64-unknown-linux-gnu")]
    GnuLinux,

    #[clap(name = "x86_64-pc-windows-msvc")]
    Windows,

    #[clap(name = "x86_64-apple-darwin")]
    MacOS,

    #[clap(skip)]
    Other,
}

impl Target {
    pub(crate) fn get_args(&self) -> Vec<String> {
        let mut args = vec![];

        if let Some(possible_value) = self.to_possible_value() {
            args.push("--target".to_string());
            args.push(possible_value.get_name().to_string());
        }

        if !self.composition_js() {
            args.push("--no-default-features".to_string());
        }
        args
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
            Target::MacOS => {
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
            }
            _ => {}
        };
        Ok(env)
    }

    fn composition_js(&self) -> bool {
        !matches!(self, Target::MuslLinux)
    }
}

impl Default for Target {
    fn default() -> Self {
        if cfg!(target_arch = "x86_64") {
            if cfg!(target_os = "windows") {
                Target::Windows
            } else if cfg!(target_os = "linux") {
                if cfg!(target_env = "gnu") {
                    Target::GnuLinux
                } else if cfg!(target_env = "musl") {
                    Target::MuslLinux
                } else {
                    Target::Other
                }
            } else if cfg!(target_os = "macos") {
                Target::MacOS
            } else {
                Target::Other
            }
        } else {
            Target::Other
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.to_possible_value() {
            Some(possible_value) => possible_value.get_name().to_string(),
            None => "unknown-target".to_string(),
        };
        write!(f, "{}", msg)
    }
}
