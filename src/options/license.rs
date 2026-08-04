use std::io::IsTerminal;

use anyhow::anyhow;
use clap::Parser;
use rover_std::prompt;
use serde::Serialize;

use crate::{RoverError, RoverErrorSuggestion, RoverResult, utils::client::StudioClientConfig};

#[cfg_attr(test, derive(Default))]
#[derive(Debug, Serialize, Parser, Clone, Copy)]
pub struct LicenseAccepter {
    /// Accept the terms and conditions of the ELv2 License without prompting for confirmation.
    /// Expected value: `accept`
    #[arg(long = "elv2-license", value_parser = license_accept, env = "APOLLO_ELV2_LICENSE")]
    pub(crate) elv2_license_accepted: Option<bool>,
}

/// What the user is being asked to accept the ELv2 license for.
///
/// Acceptance is recorded once per machine and covers every ELv2-licensed thing
/// Rover can run, so the two are interchangeable once accepted.
#[derive(Debug, Clone, Copy)]
pub enum Elv2Subject {
    /// Downloading and running an ELv2-licensed plugin binary, such as `supergraph`.
    Plugin,
    /// Composing with the ELv2-licensed composition code compiled into Rover itself. No
    /// download happens, but the licensed code still runs, so acceptance is still required.
    NativeComposition,
}

impl Elv2Subject {
    const fn prompt_preamble(self) -> &'static str {
        match self {
            Self::Plugin => {
                "By installing this plugin, you accept the terms and conditions outlined by this license."
            }
            Self::NativeComposition => {
                "By invoking composition, you accept the terms and conditions outlined by this license."
            }
        }
    }
}

impl LicenseAccepter {
    pub fn require_elv2_license(&self, client_config: &StudioClientConfig) -> RoverResult<()> {
        self.require_elv2_license_for(client_config, Elv2Subject::Plugin)
    }

    pub fn require_elv2_license_for(
        &self,
        client_config: &StudioClientConfig,
        subject: Elv2Subject,
    ) -> RoverResult<()> {
        let did_accept = self.previously_accepted(client_config)?;
        if did_accept || self.prompt_accept(client_config, subject)? {
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "This command requires that you accept the terms of the ELv2 license."
            )))
        }
    }

    fn previously_accepted(&self, client_config: &StudioClientConfig) -> RoverResult<bool> {
        Ok(
            if let Some(elv2_license_accepted) = self.elv2_license_accepted {
                if elv2_license_accepted {
                    client_config.config.remember_elv2_license_accept()?;
                    true
                } else {
                    false
                }
            } else {
                client_config.config.did_accept_elv2_license()
            },
        )
    }

    fn prompt_accept(
        &self,
        client_config: &StudioClientConfig,
        subject: Elv2Subject,
    ) -> RoverResult<bool> {
        // If we're not attached to a TTY then we can't get user input, so there's
        // nothing to do except inform the user about the `--elv2-license` flag.
        if !std::io::stdin().is_terminal() {
            let mut err = RoverError::new(anyhow!(
                "This command requires that you accept the terms of the ELv2 license."
            ));
            let mut suggestion = "Before running this command again, you need to either set `APOLLO_ELV2_LICENSE=accept` as an environment variable, or pass the `--elv2-license=accept` argument.".to_string();
            if std::env::var_os("CI").is_none() {
                suggestion.push_str(" You will only need to do this once on this machine.")
            }
            err.set_suggestion(RoverErrorSuggestion::Adhoc(suggestion));
            Err(err)
        } else {
            eprintln!("{}", subject.prompt_preamble());
            eprintln!(
                "More information on the ELv2 license can be found here: https://go.apollo.dev/elv2."
            );

            let did_accept = prompt::prompt_confirm_default_no(
                "Do you accept the terms and conditions of the ELv2 license?",
            )?;

            if did_accept {
                client_config.config.remember_elv2_license_accept()?;
            }

            Ok(did_accept)
        }
    }
}

fn license_accept(elv2_license: &str) -> std::result::Result<bool, anyhow::Error> {
    if elv2_license.eq_ignore_ascii_case("accept") {
        Ok(true)
    } else {
        Err(anyhow!("Allowed values: 'accept'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Native composition installs nothing, so the plugin wording would be misleading. The exact
    /// phrasing is free to change; not claiming an install is the part that has to hold.
    #[test]
    fn native_composition_preamble_does_not_mention_installing() {
        let preamble = Elv2Subject::NativeComposition.prompt_preamble();
        assert!(!preamble.contains("install"), "got: {preamble}");
        assert!(preamble.contains("accept"), "got: {preamble}");
    }

    #[test]
    fn plugin_preamble_still_mentions_installing() {
        assert!(Elv2Subject::Plugin.prompt_preamble().contains("installing"));
    }
}
