use saucer::{anyhow, Utf8PathBuf};

use crate::command::install::Plugin;
use crate::command::Install;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Clone)]
pub struct RouterRunner {
    read_path: Utf8PathBuf,
    opts: PluginOpts,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
}

impl RouterRunner {
    pub fn new(
        read_path: Utf8PathBuf,
        opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Self {
        Self {
            read_path,
            opts,
            override_install_path,
            client_config,
        }
    }

    pub fn get_command_to_spawn(&self) -> Result<String> {
        let plugin = Plugin::Router;
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepted: self.opts.elv2_license_accepted,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let exe = install_command
            .get_versioned_plugin(
                self.override_install_path.clone(),
                self.client_config.clone(),
                self.opts.skip_update,
            )
            .map_err(|e| anyhow!("{}", e))?;

        Ok(format!(
            "{} --supergraph {} --hot-reload",
            &exe,
            self.read_path.as_str()
        ))
    }
}
