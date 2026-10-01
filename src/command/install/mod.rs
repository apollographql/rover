use std::{convert::TryFrom, env, fmt};

use anyhow::anyhow;
use binstall::{Installer, InstallerError};
use camino::Utf8PathBuf;
use clap::Parser;
use rover_print::print::{Print, PrintExt};
use rover_std::Style;
use serde::Serialize;

#[cfg(feature = "composition-js")]
use crate::plugin::{
    automatic::AutomaticDownloads,
    discovery::{ManifestDirs, project_in_scope},
    error::{DownloadControl, RequestOrigin},
};
use crate::{
    PKG_NAME, RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
    command::{docs::shortlinks, plugin::PluginInstall},
    options::LicenseAccepter,
    utils::{client::StudioClientConfig, env::RoverEnvKey},
};

mod plugin;
// Other modules only ever construct this in their own #[cfg(test)] fixtures, never
// from production code, so this re-export is test-only rather than merely unused.
#[cfg(test)]
pub(crate) use plugin::PluginLevel;
pub(crate) use plugin::{
    McpServerVersion, Plugin, PluginInstaller, PluginProvenance, PluginProvenanceTracker,
    PluginSource, federation_version,
};

#[derive(Debug, Serialize, Parser)]
pub struct Install {
    /// Overwrite any existing binary without prompting for confirmation.
    #[arg(long = "force", short = 'f')]
    pub(crate) force: bool,

    /// Deprecated: use `rover plugin install` instead.
    #[arg(long)]
    pub(crate) plugin: Option<Plugin>,

    #[clap(flatten)]
    pub(crate) elv2_license_accepter: LicenseAccepter,
}

impl Install {
    pub async fn do_install<P: Print + ?Sized>(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        stderr: &P,
    ) -> RoverResult<RoverOutput> {
        if let (Some(plugin), Some(plugin_install)) = (&self.plugin, self.as_plugin_install()) {
            stderr.warnln(DeprecatedAlias(plugin));
            return plugin_install
                .run(override_install_path, client_config)
                .await;
        }

        let binary_name = PKG_NAME.to_string();
        let rover_installer = installer(&binary_name, self.force, override_install_path)?;
        // install rover
        let install_location = rover_installer
            .install().map_err(|e| {
                let mut err = RoverError::from(anyhow!("Could not install '{binary_name}' because {}", e.to_string().to_lowercase()));
                if matches!(e, InstallerError::NoTty) {
                    err.set_suggestion(RoverErrorSuggestion::Adhoc("Try re-running this command with the `--force` flag to overwrite the existing binary.".to_string()));
                }
                err
            })?;

        if install_location.is_some() {
            let bin_dir_path = rover_installer.get_bin_dir_path()?;
            eprintln!("{} was successfully installed. Great!", binary_name);

            if !cfg!(windows)
                && let Some(path_var) = env::var_os("PATH")
                && !path_var
                    .to_string_lossy()
                    .to_string()
                    .contains(bin_dir_path.as_str())
            {
                eprintln!(
                    "\nTo get started you need Rover's bin directory ({}) in your PATH environment variable. Next time you log in this will be done automatically.",
                    bin_dir_path
                );
                if let Ok(shell_var) = env::var("SHELL") {
                    eprintln!(
                        "\nTo configure your current shell, you can run:\nexec {} -l",
                        shell_var
                    );
                }
            }

            // these messages are duplicated in `installers/npm/binary.js`
            // for the npm installer.
            eprintln!(
                "If you would like to disable Rover's anonymized usage collection, you can set {}=true",
                RoverEnvKey::TelemetryDisabled
            );
            eprintln!(
                "You can check out our documentation at {}.",
                Style::Link.paint(shortlinks::get_url_from_slug("docs"))
            );
        } else {
            eprintln!(
                "{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.",
                binary_name
            );
        }

        Ok(RoverOutput::EmptySuccess)
    }

    /// `rover install --plugin` as the `rover plugin install` it stands in for.
    fn as_plugin_install(&self) -> Option<PluginInstall> {
        self.plugin.as_ref().map(|plugin| PluginInstall {
            plugin: Some(plugin.clone()),
            force: self.force,
            // The alias is deprecated, so it gains no new flags; the
            // environment variable still applies to it, through the verb.
            no_download: false,
            global: false,
            manifest_path: None,
            elv2_license_accepter: self.elv2_license_accepter,
        })
    }

    #[cfg(feature = "composition-js")]
    pub(crate) async fn get_versioned_plugin(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        skip_update: bool,
        automatic_downloads: AutomaticDownloads,
        origin: Option<RequestOrigin>,
    ) -> RoverResult<PluginProvenance> {
        // `--skip-update`, or `APOLLO_ROVER_SKIP_UPDATE`, means every command that
        // installs a plugin on the fly (compose, dev, lsp) uses an installed
        // plugin and never reaches for the registry, failing by name when none
        // is installed. So does having nothing opt in to automatic downloads.
        // The explicit `rover plugin install` doesn't go through here, so none
        // of them stops it downloading. See #1892.
        let downloads_disabled_by = on_the_fly_control(skip_update, automatic_downloads);
        // A plugin installed in the project in scope is used before a global
        // one (FR26). Anything downloaded still goes to the global level.
        let project = project_in_scope(ManifestDirs::for_this_process(
            override_install_path.as_deref(),
        ));
        let rover_installer = installer(PKG_NAME, self.force, override_install_path)?;
        if let Some(plugin) = &self.plugin {
            let mut installer = PluginInstaller::new(client_config, rover_installer, self.force);
            if let Some(project) = project {
                installer =
                    installer.also_looking_in(plugin::PluginLevel::Project, project.join("bin"));
            }
            installer
                .requested_by(origin)
                .without_downloads(downloads_disabled_by)
                .install(plugin)
                .await
        } else {
            let mut err =
                RoverError::new(anyhow!("Could not find a plugin to get a version from."));
            err.set_suggestion(RoverErrorSuggestion::SubmitIssue);
            Err(err)
        }
    }
}

/// FR34's warning that `rover install --plugin` is deprecated, naming the
/// `rover plugin install` that replaces it. The request is given in its modern
/// spelling, so the suggestion is one the user can copy as is.
struct DeprecatedAlias<'a>(&'a Plugin);

impl fmt::Display for DeprecatedAlias<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`rover install --plugin` is deprecated. Use `rover plugin install {}@{}` instead.",
            self.0.name(),
            self.0.request()
        )
    }
}

/// What `--skip-update` disables downloads as, if anything: the flag when it
/// was passed, or failing that, `APOLLO_ROVER_SKIP_UPDATE`.
#[cfg(feature = "composition-js")]
fn skip_update_control(flag: bool) -> Option<DownloadControl> {
    if flag {
        Some(DownloadControl::SkipUpdateFlag)
    } else if crate::utils::skip_all_updates() {
        Some(DownloadControl::SkipUpdateEnvVar)
    } else {
        None
    }
}

/// What forbids a command installing a plugin on the fly from downloading
/// it, if anything does: `--skip-update` as [`skip_update_control`] spells
/// it, or failing that, nothing having opted in to automatic downloads.
///
/// The per-invocation control comes first. A standing opt-in may permit a
/// download, but never one a run was told not to make.
#[cfg(feature = "composition-js")]
fn on_the_fly_control(
    skip_update: bool,
    automatic_downloads: AutomaticDownloads,
) -> Option<DownloadControl> {
    skip_update_control(skip_update).or_else(|| automatic_downloads.control())
}

/// The binstall installer that Rover and its plugins are installed through.
pub(crate) fn installer(
    binary_name: &str,
    force_install: bool,
    override_install_path: Option<Utf8PathBuf>,
) -> RoverResult<Installer> {
    if let Ok(executable_location) = env::current_exe() {
        let executable_location = Utf8PathBuf::try_from(executable_location)?;
        Ok(Installer {
            binary_name: binary_name.to_string(),
            force_install,
            override_install_path,
            executable_location,
            install_root: None,
        })
    } else {
        Err(anyhow!("Failed to get the current executable's path.").into())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use rstest::rstest;
    use speculoos::prelude::*;

    #[cfg(feature = "composition-js")]
    use super::{AutomaticDownloads, DownloadControl, on_the_fly_control, skip_update_control};
    use super::{DeprecatedAlias, Install};
    use crate::command::Plugins;

    #[cfg(feature = "composition-js")]
    #[rstest]
    #[case::neither(false, None, None)]
    #[case::the_variable(false, Some("true"), Some(DownloadControl::SkipUpdateEnvVar))]
    #[case::the_flag(true, None, Some(DownloadControl::SkipUpdateFlag))]
    #[case::the_flag_over_the_variable(true, Some("1"), Some(DownloadControl::SkipUpdateFlag))]
    #[case::a_variable_that_is_off(false, Some("false"), None)]
    // `APOLLO_ROVER_NO_DOWNLOAD` is set throughout: it guards only an explicit
    // install, and must not reach the on-the-fly commands.
    fn skip_update_names_the_control_the_user_spelled(
        #[case] flag: bool,
        #[case] variable: Option<&str>,
        #[case] expected: Option<DownloadControl>,
    ) {
        temp_env::with_vars(
            [
                (crate::utils::SKIP_UPDATE_ENV, variable),
                (crate::utils::NO_DOWNLOAD_ENV, Some("true")),
            ],
            || {
                assert_that!(skip_update_control(flag)).is_equal_to(expected);
            },
        );
    }

    #[cfg(feature = "composition-js")]
    #[rstest]
    #[case::not_opted_in(AutomaticDownloads::NotAllowed, Some(DownloadControl::NotOptedIn))]
    #[case::opted_in(AutomaticDownloads::Allowed, None)]
    fn without_skip_update_downloading_waits_on_the_opt_in(
        #[case] automatic_downloads: AutomaticDownloads,
        #[case] expected: Option<DownloadControl>,
    ) {
        temp_env::with_var(crate::utils::SKIP_UPDATE_ENV, None::<&str>, || {
            assert_that!(on_the_fly_control(false, automatic_downloads)).is_equal_to(expected);
        });
    }

    #[rstest]
    #[case::plain(&["supergraph@=2.9.3"])]
    #[case::forced(&["router@2", "--force"])]
    #[case::license_accepted(&["router@2", "--elv2-license", "accept"])]
    fn the_alias_runs_what_the_plugin_noun_would(#[case] args: &[&str]) {
        let (request, flags) = args.split_first().unwrap();
        let alias =
            Install::try_parse_from(["install", "--plugin", request].iter().chain(flags)).unwrap();
        let noun =
            Plugins::try_parse_from(["plugin", "install", request].iter().chain(flags)).unwrap();

        assert_that!(serde_json::to_value(alias.as_plugin_install()).unwrap())
            .is_equal_to(serde_json::to_value(noun.install()).unwrap());
    }

    #[rstest]
    #[case::exact("supergraph@=2.9.3", "supergraph@=2.9.3")]
    #[case::major("router@2", "router@2")]
    #[case::legacy_spelling("supergraph@latest-2", "supergraph@2")]
    fn the_alias_names_its_replacement(#[case] request: &str, #[case] replacement: &str) {
        let install = Install::try_parse_from(["install", "--plugin", request]).unwrap();
        let plugin = install.as_plugin_install().unwrap().plugin.unwrap();

        assert_that!(DeprecatedAlias(&plugin).to_string()).is_equal_to(format!(
            "`rover install --plugin` is deprecated. Use `rover plugin install {replacement}` instead."
        ));
    }

    #[test]
    fn bare_install_is_not_a_plugin_install() {
        let install = Install::try_parse_from(["install"]).unwrap();
        assert_that!(install.as_plugin_install()).is_none();
    }
}
