use std::io::IsTerminal;

use anyhow::anyhow;
use clap::Parser;
use serde::Serialize;

use crate::{utils::client::StudioClientConfig, RoverError, RoverErrorSuggestion, RoverResult};

use rover_std::prompt;

#[cfg_attr(test, derive(Default))]
#[derive(Debug, Serialize, Parser, Clone, Copy)]
pub struct LicenseAccepter {
    /// Accept the terms and conditions of the ELv2 License without prompting for confirmation.
    /// Expected value: `accept`
    #[arg(long = "elv2-license", value_parser = license_accept, env = "APOLLO_ELV2_LICENSE")]
    pub(crate) elv2_license_accepted: Option<bool>,
}

impl LicenseAccepter {
    pub fn require_elv2_license(&self, client_config: &StudioClientConfig) -> RoverResult<()> {
        let did_accept = self.previously_accepted(client_config)?;
        if did_accept || self.prompt_accept(client_config)? {
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

    fn prompt_accept(&self, client_config: &StudioClientConfig) -> RoverResult<bool> {
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
            eprintln!("By installing this plugin, you accept the terms and conditions outlined by this license.");
            eprintln!("More information on the ELv2 license can be found here: https://go.apollo.dev/elv2.");

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
