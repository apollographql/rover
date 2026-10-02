use std::{fmt::Display, io, process};

use camino::Utf8PathBuf;
use clap::{
    Parser, ValueEnum,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use config::{Config, Profile};
use houston as config;
use lazycell::{AtomicLazyCell, LazyCell};
use reqwest::Client;
use rover_client::shared::GitContext;
use rover_std::Style;
use serde::Serialize;
use sputnik::Session;
use timber::Level;

#[cfg(feature = "oauth")]
use crate::options::OauthOpts;
use crate::{
    RoverError, RoverResult,
    command::{self, RoverOutput},
    options::{
        DEFAULT_PROFILE, OutputOpts, PROJECT_FILE, ProfileOpt, ProfileSelection,
        ProjectSettingValue, ProjectSettings, SettingName, SettingType, SettingValueError,
    },
    utils::{
        client::{ClientBuilder, ClientTimeout, StudioClientConfig},
        env::{RoverEnv, RoverEnvKey},
        stringify::option_from_display,
        version,
    },
};

/// Clap styling
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

/// Which stored source supplied a setting's value: FR25's three middle
/// tiers, between the environment above and the built-in default below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoredTier {
    /// A profile named by `--profile`, including `--profile default`.
    ExplicitProfile,
    /// The `settings:` section of `rover.yaml`.
    ProjectFile,
    /// The `default` profile, active because `--profile` wasn't passed.
    DefaultProfile,
}

/// One stored tier's value for a setting, as stored and not yet validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredLayer {
    pub(crate) tier: StoredTier,
    pub(crate) value: StoredValue,
}

/// What a stored tier holds: a profile's string, or the project file's
/// entry with the key as its author spelled it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoredValue {
    Profile(String),
    ProjectFile(crate::options::ProjectSetting),
}

impl StoredLayer {
    /// The value as stored, for reporting rather than use.
    pub(crate) fn as_written(&self) -> String {
        match &self.value {
            StoredValue::Profile(raw) => raw.clone(),
            StoredValue::ProjectFile(setting) => setting.value.as_written(),
        }
    }
}

#[derive(Debug, Serialize, Parser)]
#[command(
    name = "Rover",
    author,
    version,
    styles = STYLES,
    about = "Rover - Your Graph Companion",
    after_help = format!("\n\n{}
    
Run the following command to authenticate with GraphOS:

    {}

Once you're authenticated, create a new graph:

    {}

To learn more about Rover, view the full documentation:

    {}
",
        Style::SuccessHeading.paint("** Getting Started with Rover **"),
        Style::Command.paint("$ rover config auth"),
        Style::Command.paint("$ rover init"),
        Style::Command.paint("$ rover docs open\n"),
    )
)]
#[command(next_line_help = true)]
pub struct Rover {
    #[clap(subcommand)]
    command: Command,

    /// Specify Rover's log level
    #[arg(long = "log", short = 'l', global = true, env = "APOLLO_LOG_LEVEL")]
    #[serde(serialize_with = "option_from_display")]
    log_level: Option<Level>,

    /// Name of configuration profile to use
    // `Option` rather than defaulted here, so `--profile default` typed
    // literally can be told apart from omitting the flag.
    #[arg(long = "profile", global = true)]
    #[serde(skip_serializing)]
    profile_name: Option<String>,

    #[clap(flatten)]
    output_opts: OutputOpts,

    /// Accept invalid certificates when performing HTTPS requests.
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If invalid certificates are trusted, any certificate for any site will be trusted for use.
    /// This includes expired certificates.
    /// This introduces significant vulnerabilities, and should only be used as a last resort.
    #[arg(long = "insecure-accept-invalid-certs", global = true)]
    accept_invalid_certs: bool,

    /// Accept invalid hostnames when performing HTTPS requests.
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If hostname verification is not used, any valid certificate for any site will be trusted for use from any other.
    /// This introduces a significant vulnerability to man-in-the-middle attacks.
    #[arg(long = "insecure-accept-invalid-hostnames", global = true)]
    accept_invalid_hostnames: bool,

    /// Configure the timeout length (in seconds) when performing HTTP(S) requests.
    ///
    /// Defaults to 30s for standard operations and 300s for plugin downloads.
    #[arg(long = "client-timeout", global = true, env = "APOLLO_CLIENT_TIMEOUT")]
    client_timeout: Option<ClientTimeout>,

    /// Override the GraphOS registry endpoint.
    #[arg(long = "registry-url", global = true, env = "APOLLO_REGISTRY_URL")]
    registry_url: Option<String>,

    /// Override the endpoint anonymous usage telemetry is reported to.
    #[arg(long = "telemetry-url", global = true, env = "APOLLO_TELEMETRY_URL")]
    telemetry_url: Option<String>,

    /// Opt out of anonymous usage telemetry.
    ///
    /// The `APOLLO_TELEMETRY_DISABLED` environment variable disables
    /// telemetry on any value it's set to, including `false` - this flag
    /// doesn't change that, it's just an additional way to opt out.
    #[arg(long = "telemetry-disabled", global = true)]
    telemetry_disabled: bool,

    /// Override how long check/launch polling waits (in whole seconds) before giving up.
    #[arg(
        long = "checks-timeout",
        global = true,
        env = "APOLLO_CHECKS_TIMEOUT_SECONDS"
    )]
    checks_timeout: Option<u64>,

    /// Override the host plugin binaries (the `router` and `supergraph` composition plugins) are downloaded from.
    #[arg(
        long = "download-host",
        global = true,
        env = "APOLLO_ROVER_DOWNLOAD_HOST"
    )]
    download_host: Option<String>,

    /// Override the Git remote URL reported to GraphOS on check/publish.
    ///
    /// Defaults to the remote URL inferred from the current directory's Git
    /// repository.
    #[arg(long = "vcs-remote-url", global = true, env = "APOLLO_VCS_REMOTE_URL")]
    vcs_remote_url: Option<String>,

    /// Override the Git branch reported to GraphOS on check/publish.
    ///
    /// Defaults to the branch inferred from the current directory's Git
    /// repository.
    #[arg(long = "vcs-branch", global = true, env = "APOLLO_VCS_BRANCH")]
    vcs_branch: Option<String>,

    /// Override the Git commit reported to GraphOS on check/publish.
    ///
    /// Defaults to the commit inferred from the current directory's Git
    /// repository.
    #[arg(long = "vcs-commit", global = true, env = "APOLLO_VCS_COMMIT")]
    vcs_commit: Option<String>,

    /// Override the Git commit author reported to GraphOS on check/publish.
    ///
    /// Defaults to the author inferred from the current directory's Git
    /// repository.
    #[arg(long = "vcs-author", global = true, env = "APOLLO_VCS_AUTHOR")]
    vcs_author: Option<String>,

    /// Override the location of Rover's config directory, where profiles live.
    #[arg(long = "config-home", global = true, env = "APOLLO_CONFIG_HOME")]
    config_home: Option<Utf8PathBuf>,

    /// Override the location Rover installs its binary and plugins to.
    #[arg(long = "rover-home", global = true, env = "APOLLO_HOME")]
    rover_home: Option<Utf8PathBuf>,

    /// Skip checking for newer versions of rover.
    ///
    /// Set the `APOLLO_ROVER_SKIP_UPDATE` environment variable (to `1` or `true`)
    /// to disable all of Rover's auto-updating at once — both this self-update
    /// check and the `supergraph`/`router` plugin auto-updates (`--skip-update`).
    #[arg(long = "skip-update-check", global = true)]
    skip_update_check: bool,

    /// Suppress the notices Rover prints when a profile or the project file
    /// overrides a network destination, or when an environment variable
    /// overrides a value an explicitly selected profile or the project file
    /// also set.
    ///
    /// Set the `APOLLO_ROVER_NO_CONFIG_NOTICES` environment variable (to
    /// `1` or `true`) to suppress them the same way. Neither a profile nor
    /// a project file can suppress these notices - only this flag or its
    /// environment variable can.
    #[arg(long = "no-config-notices", global = true)]
    no_config_notices: bool,

    #[cfg(feature = "oauth")]
    #[clap(flatten)]
    oauth_opts: OauthOpts,

    #[arg(skip)]
    #[serde(skip_serializing)]
    env_store: LazyCell<RoverEnv>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    client_builder: AtomicLazyCell<ClientBuilder>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    client: AtomicLazyCell<Client>,

    /// Settings this invocation has already printed a §3.9 override notice
    /// for - at most one notice per setting per process (FR61), keyed by
    /// canonical setting name.
    #[arg(skip)]
    #[serde(skip_serializing)]
    noticed_settings: std::sync::Mutex<std::collections::HashSet<&'static str>>,

    /// The project file's `settings:` section, discovered and read on first
    /// use (see `project_settings`).
    #[arg(skip)]
    #[serde(skip_serializing)]
    project_settings: std::sync::OnceLock<ProjectSettings>,

    /// Where a test's manifests live, in place of discovery from the
    /// process's working directory - which parallel tests can't each change.
    /// `None` means no manifest at either level, so a test never reads the
    /// real working directory's `.rover/`.
    #[cfg(test)]
    #[arg(skip)]
    #[serde(skip_serializing)]
    test_manifest_dirs: Option<crate::plugin::discovery::ManifestDirs>,
}

impl Rover {
    pub async fn run_from_args() -> RoverResult<()> {
        Rover::parse().run().await
    }

    pub async fn run(&self) -> RoverResult<()> {
        timber::init(self.log_level);
        tracing::trace!(command_structure = ?self);
        self.output_opts.set_no_color();

        // attempt to create a new `Session` to capture anonymous usage data
        let rover_output = match Session::new(self) {
            // if successful, report the usage data in the background
            Ok(session) => {
                // kicks off the reporting on a background thread
                let report_thread = tokio::task::spawn(async move {
                    // log + ignore errors because it is not in the critical path
                    let _ = session.report().await.map_err(|telemetry_error| {
                        tracing::debug!(?telemetry_error);
                        telemetry_error
                    });
                });

                // kicks off the app on the main thread
                // don't return an error with ? quite yet
                // since we still want to report the usage data
                let app_result = self.execute_command().await;

                // makes sure the reporting finishes in the background
                // before continuing.
                // ignore errors because it is not in the critical path
                let _ = report_thread.await;

                // return result of app execution
                // now that we have reported our usage data
                app_result
            }

            // otherwise just run the app without reporting
            Err(_) => self.execute_command().await,
        };

        match rover_output {
            Ok(output) => {
                let exit_code = output.exit_code();
                self.output_opts.handle_output(output)?;
                process::exit(exit_code);
            }
            Err(error) => {
                self.output_opts.handle_output(error)?;
                process::exit(1);
            }
        }
    }

    pub async fn execute_command(&self) -> RoverResult<RoverOutput> {
        // A project file that names a credential or spells one setting both
        // ways fails every command run in the project before it sends any
        // request - the update check's included (FR70, FR73). One that's
        // merely carrying keys Rover doesn't apply warns about each of them
        // below (FR72, FR76).
        let project_settings = self.project_settings()?;

        // before running any commands, we check if rover is up to date
        // this only happens once a day automatically
        // we skip this check for the `rover update` commands, since they
        // do their own checks.
        // the check is also skipped if the `--skip-update-check` flag is passed.
        if let Command::Update(_) = &self.command { /* skip check */
        } else if !self.skip_update_check && !crate::utils::skip_all_updates() {
            let config = self.get_rover_config();
            if let Ok(config) = config {
                let _ =
                    version::check_for_update(config, false, self.get_reqwest_client(None)?).await;
            }
        }

        let profile_opt = self.get_profile_opt();
        for message in self.unrecognized_setting_warnings(&profile_opt) {
            self.print_unrecognized_setting_warning(message);
        }
        for message in project_settings.warnings() {
            self.print_unrecognized_setting_warning(message.clone());
        }

        match &self.command {
            Command::Init(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            Command::Completion(command) => command.run(),
            Command::Config(command) => command.run(&profile_opt, self).await,
            #[cfg(feature = "oauth")]
            Command::Auth(command) => {
                command
                    .run(
                        self.get_client_config().await?,
                        self.get_oauth_config(&command.oauth_settings_used())?,
                        &profile_opt,
                    )
                    .await
            }
            #[cfg(feature = "composition-js")]
            Command::Connector(command) => {
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config().await?,
                        &profile_opt,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
            Command::Contract(command) => {
                command
                    .run(self.get_client_config().await?, self, &profile_opt)
                    .await
            }
            Command::Schema(command) => command.run(self.get_client_config().await?).await,
            Command::Dev(command) => {
                // Must run before `resolve_graph_ref_setting` below, not
                // inside `Dev::run` - `Rover`'s env-var cache (`env_store`,
                // a `LazyCell<RoverEnv>`) is filled on first `get_env_var`
                // call and never refreshed, so a `.env`-supplied
                // `APOLLO_GRAPH_REF` loaded any later would be invisible to
                // it, letting a stored profile value wrongly win instead
                // (inverting FR25's tier order) and spuriously triggering
                // the "running without GraphOS credentials" notice.
                dotenvy::dotenv().ok();
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config().await?,
                        self.log_level,
                        &profile_opt,
                        &rover_print::print::stderr::default(),
                        self.resolve_graph_ref_setting()?,
                    )
                    .await
            }
            Command::Supergraph(command) => {
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config().await?,
                        self.output_opts.output_file.clone(),
                        &profile_opt,
                    )
                    .await
            }
            Command::Docs(command) => command.run(),
            Command::Graph(command) => {
                command
                    .run(
                        self.get_client_config().await?,
                        self.get_git_context()?,
                        self,
                        &self.output_opts,
                        &profile_opt,
                    )
                    .await
            }
            Command::Template(command) => command.run(self).await,
            Command::Readme(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            Command::Subgraph(command) => {
                command
                    .run(
                        self.get_client_config().await?,
                        self.get_git_context()?,
                        self,
                        &self.output_opts,
                        &profile_opt,
                    )
                    .await
            }
            Command::Update(command) => {
                command
                    .run(self.get_rover_config()?, self.get_reqwest_client(None)?)
                    .await
            }
            Command::Install(command) => {
                command
                    .do_install(
                        self.get_install_override_path()?,
                        self.get_client_config().await?,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
            Command::Plugin(command) => {
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config().await?,
                    )
                    .await
            }
            Command::Info(command) => command.run(),
            Command::Explain(command) => command.run(),
            Command::PersistedQueries(command) => {
                let client_config = if command.requires_client_config() {
                    Some(self.get_client_config().await?)
                } else {
                    None
                };
                command
                    .run(client_config, &profile_opt, &rover_print::stderr::default())
                    .await
            }
            Command::License(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            #[cfg(feature = "composition-js")]
            Command::Lsp(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            Command::ApiKeys(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            Command::Client(command) => {
                command
                    .run(
                        self.get_client_config().await?,
                        self.get_git_context()?,
                        &profile_opt,
                    )
                    .await
            }
            Command::GraphArtifact(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
        }
    }

    /// Resolves the active profile from the global `--profile` flag, exactly
    /// once, the same way for every command - including commands that have
    /// no use for a credential (they accept the flag via `global = true` and
    /// simply don't call this).
    pub(crate) fn get_profile_opt(&self) -> ProfileOpt {
        ProfileOpt {
            profile_name: self
                .profile_name
                .clone()
                .unwrap_or_else(|| DEFAULT_PROFILE.to_string()),
            selection: if self.profile_name.is_some() {
                ProfileSelection::Explicit
            } else {
                ProfileSelection::Default
            },
        }
    }

    /// The raw, clap-merged `--registry-url`/`APOLLO_REGISTRY_URL` value
    /// (flag beats env, whichever supplied it) - before the stored tiers
    /// (profiles and the project file) apply. `rover config show` (`src/command/config/show/mod.rs`) uses this
    /// to tell a flag/env source apart from a profile one; every other
    /// caller wants `get_client_config`'s fully-resolved value instead.
    pub(crate) fn registry_url_flag_or_env(&self) -> Option<String> {
        self.registry_url.clone()
    }

    /// The raw, clap-merged `--telemetry-url`/`APOLLO_TELEMETRY_URL` value,
    /// before the stored tiers apply. See `registry_url_flag_or_env`.
    pub(crate) fn telemetry_url_flag_or_env(&self) -> Option<String> {
        self.telemetry_url.clone()
    }

    /// Whether `--telemetry-disabled` was passed, before the stored tiers
    /// apply. See `registry_url_flag_or_env`.
    pub(crate) const fn telemetry_disabled_flag(&self) -> bool {
        self.telemetry_disabled
    }

    /// The raw, clap-merged `--download-host`/`APOLLO_ROVER_DOWNLOAD_HOST`
    /// value, before the stored tiers apply. See `registry_url_flag_or_env`.
    pub(crate) fn download_host_flag_or_env(&self) -> Option<String> {
        self.download_host.clone()
    }

    /// The resolved `--telemetry-url`/`APOLLO_TELEMETRY_URL` override, for
    /// `impl Report for Rover` (`src/utils/telemetry.rs`) - a different
    /// module, so it can't reach the private field directly.
    ///
    /// Telemetry reporting is already best-effort and fully isolated from
    /// the invocation's own exit code (see `run`'s `report_thread`), so an
    /// invalid stored value here is logged and skipped rather than failing a
    /// command telemetry has nothing to do with - unlike `resolve_setting`'s
    /// normal contract, this never fails.
    pub(crate) fn telemetry_url_override(&self) -> Option<String> {
        let resolved =
            match self.resolve_setting(self.telemetry_url.clone(), SettingName::TelemetryUrl) {
                Ok(value) => value,
                Err(error) => {
                    tracing::debug!(?error, "ignoring invalid stored APOLLO_TELEMETRY_URL");
                    None
                }
            };
        // Nothing is ever sent to this URL when telemetry is disabled, so a
        // notice about it would name a setting the invocation never uses.
        // `is_telemetry_disabled()` alone misses the bare-env-var case
        // (mirrors `sputnik::Session::is_telemetry_enabled`'s own check).
        let telemetry_disabled = self.is_telemetry_disabled()
            || self
                .get_env_var(RoverEnvKey::TelemetryDisabled)
                .unwrap_or_default()
                .is_some();
        if !telemetry_disabled {
            match self.config_override_notice(
                SettingName::TelemetryUrl,
                self.telemetry_url.as_deref(),
                self.get_env_var(RoverEnvKey::TelemetryUrl)
                    .unwrap_or_default()
                    .as_deref(),
                resolved.as_deref(),
            ) {
                Ok(Some(message)) => self.print_config_notice(message),
                Ok(None) => {}
                Err(error) => {
                    tracing::debug!(?error, "failed to check for a APOLLO_TELEMETRY_URL notice");
                }
            }
        }
        resolved
    }

    /// The resolved `--telemetry-disabled` flag plus its stored tiers, for
    /// `impl Report for Rover` (`src/utils/telemetry.rs`).
    /// `APOLLO_TELEMETRY_DISABLED` keeps its own, separate presence-only
    /// check (`RoverEnvKey::TelemetryDisabled`) as its *environment
    /// variable's* parsing (FR21) - this only adds the profile and project
    /// file tiers beneath it, where a stored value is a typed boolean (FR24).
    /// Errors are swallowed the same way and for the same reason as
    /// `telemetry_url_override`.
    pub(crate) fn is_telemetry_disabled(&self) -> bool {
        if self.telemetry_disabled {
            return true;
        }
        let resolved = match self.resolve_stored_setting(SettingName::TelemetryDisabled) {
            Ok(Some((_, value))) => value.eq_ignore_ascii_case("true"),
            Ok(None) => false,
            Err(error) => {
                tracing::debug!(?error, "ignoring invalid stored APOLLO_TELEMETRY_DISABLED");
                false
            }
        };
        match self.telemetry_disabled_override_notice() {
            Ok(Some(message)) => self.print_config_notice(message),
            Ok(None) => {}
            Err(error) => {
                tracing::debug!(
                    ?error,
                    "failed to check for a APOLLO_TELEMETRY_DISABLED notice"
                );
            }
        }
        resolved
    }

    /// `APOLLO_TELEMETRY_DISABLED` is handled separately from
    /// `config_override_notice` because the env var is presence-only - any
    /// value set means disabled. Returns a notice message if the env var
    /// overrides an explicit profile's setting, `None` otherwise. Caller
    /// must print whatever `Some` this returns.
    fn telemetry_disabled_override_notice(&self) -> RoverResult<Option<String>> {
        if self.config_notices_suppressed() || self.telemetry_disabled {
            return Ok(None);
        }
        let name = SettingName::TelemetryDisabled;
        if self.get_env_var(RoverEnvKey::TelemetryDisabled)?.is_none() {
            return Ok(None);
        }
        if !self.mark_noticed(name) {
            return Ok(None);
        }

        let houston_config = self.get_rover_config_read_only()?;
        // As in `config_override_notice`, overriding the default profile
        // isn't surfaced; overriding an explicit profile or the project file
        // is.
        let Some((tier @ (StoredTier::ExplicitProfile | StoredTier::ProjectFile), stored_raw)) =
            self.stored_raw_with(&houston_config, name)?
        else {
            return Ok(None);
        };
        // The env var disables telemetry on any value it's set to, so it
        // only overrides anything when the stored value wasn't already
        // disabling it - one that already stores `true` sees no change to
        // notice about.
        Ok((!stored_raw.eq_ignore_ascii_case("true")).then(|| {
            format!(
                "`{name}` from the environment overrides the value set in {source}.",
                source = self.stored_tier_label(tier),
            )
        }))
    }

    pub(crate) fn get_rover_config(&self) -> RoverResult<Config> {
        let override_api_key = self.get_env_var(RoverEnvKey::Key)?;
        Ok(Config::new(self.config_home.as_ref(), override_api_key)?)
    }

    /// Like [`Rover::get_rover_config`], but never creates the config home -
    /// for read-only callers (`rover config show`, FR18) that must report
    /// what's there without establishing anything that wasn't already
    /// present.
    pub(crate) fn get_rover_config_read_only(&self) -> RoverResult<Config> {
        let override_api_key = self.get_env_var(RoverEnvKey::Key)?;
        Ok(Config::read_only(
            self.config_home.as_ref(),
            override_api_key,
        )?)
    }

    /// Resolves one setting's effective raw value, adding the stored tiers
    /// beneath an already-resolved explicit flag/environment-variable value
    /// (FR25). `Ok(None)` means neither `explicit` nor any stored tier
    /// supplied a value, so the caller falls through to its own built-in
    /// default. A stored value that fails validation fails the command
    /// outright (FR39/FR83) rather than falling through.
    fn resolve_setting(
        &self,
        explicit: Option<String>,
        name: SettingName,
    ) -> RoverResult<Option<String>> {
        if explicit.is_some() {
            return Ok(explicit);
        }
        Ok(self.resolve_stored_setting(name)?.map(|(_, value)| value))
    }

    /// Every stored tier that supplies `name`, highest precedence first, each
    /// as stored and unvalidated - the one place FR25's middle three tiers
    /// are ordered. An explicitly selected profile beats the project file,
    /// which beats the default profile; the active profile is exactly one of
    /// the two, so only one profile tier is ever consulted. Precedence is by
    /// presence alone (FR27), and a setting that isn't project-eligible never
    /// reaches the project file's settings at all (FR31). The winner is the
    /// first; `rover config show` reports the rest as what it beat (FR52/FR53,
    /// FR104).
    pub(crate) fn stored_layers_with(
        &self,
        houston_config: &Config,
        name: SettingName,
    ) -> RoverResult<Vec<StoredLayer>> {
        let profile = self.get_profile_opt();
        let profile_layer = Profile::new(&profile.profile_name, houston_config)
            .get_setting(name.as_str())?
            .map(|raw| StoredLayer {
                tier: if profile.selection.is_explicit() {
                    StoredTier::ExplicitProfile
                } else {
                    StoredTier::DefaultProfile
                },
                value: StoredValue::Profile(raw),
            });
        let project_layer = self
            .project_settings()?
            .get(name)
            .map(|setting| StoredLayer {
                tier: StoredTier::ProjectFile,
                value: StoredValue::ProjectFile(setting.clone()),
            });
        let layers = if profile.selection.is_explicit() {
            [profile_layer, project_layer]
        } else {
            [project_layer, profile_layer]
        };
        Ok(layers.into_iter().flatten().collect())
    }

    /// The winning stored tier's value for `name`, validated against its
    /// type, together with that tier. See `stored_layers_with` for the order
    /// and `resolve_setting` for where this fits beneath flags and the
    /// environment. Builds its own `Config` (creating the config home if it's
    /// missing, per `get_rover_config`'s normal contract) - callers that must
    /// not create anything (`rover config show`, FR18) use
    /// [`Rover::resolve_stored_setting_with`] with their own `Config` instead.
    pub(crate) fn resolve_stored_setting(
        &self,
        name: SettingName,
    ) -> RoverResult<Option<(StoredTier, String)>> {
        let houston_config = self.get_rover_config()?;
        self.resolve_stored_setting_with(&houston_config, name)
    }

    /// Like [`Rover::resolve_stored_setting`], but against a `Config` the
    /// caller already has, rather than building one (and possibly creating
    /// the config home) itself.
    pub(crate) fn resolve_stored_setting_with(
        &self,
        houston_config: &Config,
        name: SettingName,
    ) -> RoverResult<Option<(StoredTier, String)>> {
        let Some(winner) = self
            .stored_layers_with(houston_config, name)?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let value = match winner.value {
            StoredValue::Profile(raw) => self.validate_profile_value(name, raw)?,
            StoredValue::ProjectFile(setting) => validate_project_value(name, &setting)?,
        };
        Ok(Some((winner.tier, value)))
    }

    /// Resolves `APOLLO_TEMPLATES_API` (FR1), printing the FR64 override
    /// notice when a profile value takes effect - `--templates-api` is
    /// scoped to `rover template`'s own opts struct rather than living on
    /// `Rover`, so `Template::run` calls this directly instead of going
    /// through a `Rover`-level flag accessor the way every other
    /// network-destination setting's resolution does.
    pub(crate) fn resolve_templates_api(
        &self,
        explicit: Option<String>,
    ) -> RoverResult<Option<String>> {
        let resolved = self.resolve_setting(explicit.clone(), SettingName::TemplatesApi)?;
        if let Some(message) = self.config_override_notice(
            SettingName::TemplatesApi,
            explicit.as_deref(),
            self.get_env_var(RoverEnvKey::TemplatesApi)?.as_deref(),
            resolved.as_deref(),
        )? {
            self.print_config_notice(message);
        }
        Ok(resolved)
    }

    /// The active profile's stored value for `name`, validated against its
    /// type, whichever stored tier that profile occupies. Most callers want
    /// [`Rover::resolve_stored_setting_with`], which puts the project file in
    /// its place relative to the profile.
    pub(crate) fn resolve_profile_setting_with(
        &self,
        houston_config: &Config,
        name: SettingName,
    ) -> RoverResult<Option<String>> {
        let profile = self.get_profile_opt();
        let profile_handle = Profile::new(&profile.profile_name, houston_config);
        let Some(raw) = profile_handle.get_setting(name.as_str())? else {
            return Ok(None);
        };
        self.validate_profile_value(name, raw).map(Some)
    }

    /// `raw`, read from the active profile, validated against `name`'s type,
    /// with FR84's profile text when it fails.
    fn validate_profile_value(&self, name: SettingName, raw: String) -> RoverResult<String> {
        let profile = self.get_profile_opt();
        let value = name.setting_type().validate(raw).map_err(|error| {
            let raw = error.input();
            let message = format!(
                "`{name}` in profile `{profile_name}` is set to `{raw}`, which {reason} Run \
                `rover config set {name} {placeholder} --profile {profile_name}` to correct it.",
                profile_name = profile.profile_name,
                reason = describe_invalid_value(&error),
                placeholder = value_placeholder(&error),
            );
            // Keeps `error`'s real type in the chain (unlike
            // `anyhow::anyhow!("{error}")`, which would format it into a new,
            // untyped error) so `RoverErrorMetadata` can still downcast to
            // `SettingValueError` and assign E054 (FR86) on this read path,
            // not just `Set::run`'s write-time one.
            RoverError::new(anyhow::Error::new(error).context(message))
        })?;
        Ok(value)
    }

    /// Returns the notice message to print when `name`'s value comes from a
    /// non-default source the user should know about, or `None` if no
    /// notice applies. Fires at most once per setting per process (FR61) -
    /// callers must print whatever `Some` this returns, since the once-only
    /// gate is consumed before returning. Returns `Ok(None)` when suppressed
    /// (FR63) too.
    ///
    /// - `explicit`: the flag/env value before profile resolution (`None` if
    ///   neither supplied one)
    /// - `raw_env`: the env var's raw value, independent of what clap
    ///   resolved
    /// - `resolved`: the setting's final effective value
    ///
    /// FR60 requires the notice to fire only when the invocation actually
    /// *uses* the setting - for a network-destination setting, only once a
    /// request is actually sent to it. Rover's request-sending paths
    /// (`rover-client`'s legacy `GraphQLClient` and its newer Tower
    /// `service.rs` operations) don't share one choke point today, so
    /// hooking this in there would be a materially larger and riskier
    /// change than this slice takes on. Callers instead call this at the
    /// point a setting's value is resolved for use by a command that needs
    /// it, which over-approximates FR60: it can decide a notice for an
    /// invocation that resolves a setting but then fails before ever
    /// sending a request. Closing that gap needs a shared choke point in
    /// `rover-client` and is left for follow-up work.
    ///
    /// For `APOLLO_REGISTRY_URL` specifically, this over-approximation goes
    /// further still: `get_client_config` decides the notice for every
    /// `config` subcommand that builds a client config (`list`, `delete`,
    /// `clear`, `auth`), not just commands that go on to contact the
    /// registry. A local-only `config` verb can print a registry-URL notice
    /// for a setting that invocation never uses at all.
    fn config_override_notice(
        &self,
        name: SettingName,
        explicit: Option<&str>,
        raw_env: Option<&str>,
        resolved: Option<&str>,
    ) -> RoverResult<Option<String>> {
        if self.config_notices_suppressed() {
            return Ok(None);
        }
        if !self.mark_noticed(name) {
            return Ok(None);
        }

        let houston_config = self.get_rover_config()?;
        let is_non_default_destination = |value: &str| {
            name.is_network_destination() && Some(value) != name.builtin_default().as_deref()
        };
        // The stored tier beneath the flag/environment: what an environment
        // value overrode, or what supplied `resolved` when nothing did.
        let Some((tier, stored_raw)) = self.stored_raw_with(&houston_config, name)? else {
            // Nothing stored: only a non-default value can have come from a
            // profile the caller resolved some other way, so keep the
            // profile wording for it.
            return Ok(resolved
                .filter(|_| explicit.is_none())
                .filter(|value| is_non_default_destination(value))
                .map(|value| {
                    format!(
                        "profile `{profile_name}` sets `{name}` to `{value}`.",
                        profile_name = self.get_profile_opt().profile_name,
                    )
                }));
        };
        let source = self.stored_tier_label(tier);

        let message = if let Some(explicit_value) = explicit {
            // Credits the environment whenever its current value matches
            // what actually resolved, even if the flag (not the env var)
            // supplied that same value - harmless (the message just names
            // the wrong of two identical sources), not corrected here. The
            // environment overriding the default profile isn't surfaced
            // (FR59(a) names only an explicitly selected one); overriding the
            // project file is, the same way.
            let surfaced = matches!(tier, StoredTier::ExplicitProfile | StoredTier::ProjectFile);
            (surfaced && raw_env == Some(explicit_value)).then(|| {
                if is_non_default_destination(&stored_raw) {
                    format!(
                        "`{name}` from the environment is set to `{explicit_value}`, overriding \
                        the value set in {source}."
                    )
                } else {
                    format!("`{name}` from the environment overrides the value set in {source}.")
                }
            })
        } else {
            // FR32: an explicit profile outranking the project file isn't an
            // override, so only the winner is ever named.
            resolved
                .filter(|value| is_non_default_destination(value))
                .map(|value| format!("{source} sets `{name}` to `{value}`."))
        };

        Ok(message)
    }

    /// Whether `--no-config-notices` or `APOLLO_ROVER_NO_CONFIG_NOTICES`
    /// suppress configuration override notices (FR63). Neither a profile
    /// nor a project file can suppress them - only these two, non-persisted
    /// sources can.
    fn config_notices_suppressed(&self) -> bool {
        self.no_config_notices || crate::utils::config_notices_suppressed_by_env()
    }

    /// The winning stored tier for `name` and its value as stored - not
    /// validated, since a notice only ever describes a value the caller has
    /// already resolved (or one the environment overrode, which is never
    /// read for use).
    fn stored_raw_with(
        &self,
        houston_config: &Config,
        name: SettingName,
    ) -> RoverResult<Option<(StoredTier, String)>> {
        Ok(self
            .stored_layers_with(houston_config, name)?
            .first()
            .map(|layer| (layer.tier, layer.as_written())))
    }

    /// How a notice names a stored tier: "profile `staging`" (FR64, FR66)
    /// or "`rover.yaml`" (FR65).
    fn stored_tier_label(&self, tier: StoredTier) -> String {
        match tier {
            StoredTier::ExplicitProfile | StoredTier::DefaultProfile => format!(
                "profile `{profile_name}`",
                profile_name = self.get_profile_opt().profile_name
            ),
            StoredTier::ProjectFile => format!("`{PROJECT_FILE}`"),
        }
    }

    /// Records that `name` has had its one notice-worth-of-attention for
    /// this process (FR61), returning `true` the first time and `false`
    /// every time after.
    fn mark_noticed(&self, name: SettingName) -> bool {
        self.noticed_settings
            .lock()
            .expect("noticed_settings mutex poisoned")
            .insert(name.as_str())
    }

    fn print_config_notice(&self, message: String) {
        use rover_print::{print::Print, style::StyledText};

        rover_print::print::stderr::default().print(&StyledText::plain(format!("Note: {message}")));
    }

    /// Prints the FR38 warning text a call to `unrecognized_setting_warnings`
    /// decided on.
    fn print_unrecognized_setting_warning(&self, message: String) {
        use rover_print::{print::Print, style::StyledText};

        rover_print::print::stderr::default().print(&StyledText::plain(message));
    }

    /// Decides the FR38 warning text for any setting `profile` carries that
    /// this version of Rover doesn't recognize - so a config directory
    /// shared between Rover versions doesn't break the older one. The
    /// decision is split from printing so it can be unit tested without
    /// depending on `rover-print`'s real terminal writer; the caller must
    /// print every message this returns.
    ///
    /// Meant to run once per invocation, for every command, regardless of
    /// whether it goes on to use any setting - the same "every part of the
    /// invocation" scope as the active profile resolution this rides
    /// alongside (FR15). Best-effort: a config-home or read failure here is
    /// either reported elsewhere (when the command that follows also needs
    /// the profile) or harmless to skip (a command that doesn't touch
    /// settings at all).
    ///
    /// Uses `get_rover_config_read_only` (not `get_rover_config`) because
    /// this runs before *every* command, including read-only ones like
    /// `rover config show` (FR18/FR57) - creating the config home here would
    /// undo that verb's own no-creation guarantee.
    ///
    /// `#[cfg(feature = "oauth")]` `SettingName` variants don't exist in a
    /// non-oauth build, so a config directory written by an oauth-enabled
    /// build and read by a non-oauth build of the same version warns about
    /// settings that build simply can't compile in - not the "future
    /// version" case FR38 is meant to catch. Low impact, not corrected here.
    fn unrecognized_setting_warnings(&self, profile: &ProfileOpt) -> Vec<String> {
        let Ok(houston_config) = self.get_rover_config_read_only() else {
            return Vec::new();
        };
        let Ok(settings) = Profile::new(&profile.profile_name, &houston_config).settings() else {
            return Vec::new();
        };
        settings
            .keys()
            .filter(|key| key.parse::<SettingName>().is_err())
            .map(|key| {
                format!(
                    "Warning: profile `{profile_name}` sets `{key}`, which this version of \
                    Rover doesn't recognize. It will be ignored.",
                    profile_name = profile.profile_name,
                )
            })
            .collect()
    }

    /// The project file's classified `settings:` section, read once per
    /// process from the manifest the plugin system's discovery finds from
    /// the working directory (FR78) - empty when there's no project. Fails
    /// when the project manifest can't be read at all, or its `settings:`
    /// section names a credential or spells one setting both ways.
    pub(crate) fn project_settings(&self) -> RoverResult<&ProjectSettings> {
        if let Some(settings) = self.project_settings.get() {
            return Ok(settings);
        }
        let settings = ProjectSettings::load(&self.manifest_dirs())?;
        Ok(self.project_settings.get_or_init(|| settings))
    }

    /// Both manifest levels for this invocation: the project found from the
    /// process's working directory, and the user level under `--rover-home`
    /// (`APOLLO_HOME`) or the home directory, as plugins use. See
    /// [`manifest_dirs_for`] for the rule itself.
    #[cfg(not(test))]
    fn manifest_dirs(&self) -> crate::plugin::discovery::ManifestDirs {
        let home = directories_next::BaseDirs::new()
            .and_then(|dirs| Utf8PathBuf::from_path_buf(dirs.home_dir().to_path_buf()).ok());
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|cwd| Utf8PathBuf::from_path_buf(cwd).ok());
        manifest_dirs_for(cwd.as_deref(), self.rover_home.as_deref(), home.as_deref())
    }

    /// Unit tests never discover from the process's working directory: it's
    /// shared by every test running in parallel, and on a developer's machine
    /// it could sit under a real `rover.yaml` whose settings (or
    /// credential) would leak into every test. A test that wants a project
    /// points at a temp tree with `set_manifest_dirs`, usually through
    /// [`manifest_dirs_for`], so the real rule is still what runs.
    #[cfg(test)]
    fn manifest_dirs(&self) -> crate::plugin::discovery::ManifestDirs {
        self.test_manifest_dirs
            .clone()
            .unwrap_or(crate::plugin::discovery::ManifestDirs {
                global: None,
                project: None,
            })
    }

    /// Points this test's manifest discovery at `dirs` instead of nothing.
    #[cfg(test)]
    pub(crate) fn set_manifest_dirs(&mut self, dirs: crate::plugin::discovery::ManifestDirs) {
        self.test_manifest_dirs = Some(dirs);
    }

    /// The raw, clap-merged flag/env value of one OAuth setting, before the
    /// stored tiers apply. See `registry_url_flag_or_env`.
    #[cfg(feature = "oauth")]
    pub(crate) fn oauth_flag_or_env(&self, name: SettingName) -> Option<String> {
        let opts = &self.oauth_opts;
        match name {
            SettingName::OauthAuthorizationUrl => {
                opts.authorization_url.as_ref().map(url::Url::to_string)
            }
            SettingName::OauthTokenUrl => opts.token_url.as_ref().map(url::Url::to_string),
            SettingName::OauthDeviceAuthorizationUrl => opts
                .device_authorization_url
                .as_ref()
                .map(url::Url::to_string),
            SettingName::OauthRevocationUrl => {
                opts.revocation_url.as_ref().map(url::Url::to_string)
            }
            SettingName::OauthWhoamiUrl => opts.whoami_url.as_ref().map(url::Url::to_string),
            SettingName::OauthClientId => opts.client_id.clone(),
            _ => None,
        }
    }

    /// The env var backing each OAuth setting, for the override notice's
    /// "from the environment" case and for `config show`.
    #[cfg(feature = "oauth")]
    pub(crate) const fn oauth_env_key(name: SettingName) -> RoverEnvKey {
        match name {
            SettingName::OauthAuthorizationUrl => RoverEnvKey::OauthAuthorizationUrl,
            SettingName::OauthTokenUrl => RoverEnvKey::OauthTokenUrl,
            SettingName::OauthDeviceAuthorizationUrl => RoverEnvKey::OauthDeviceAuthorizationUrl,
            SettingName::OauthRevocationUrl => RoverEnvKey::OauthRevocationUrl,
            SettingName::OauthWhoamiUrl => RoverEnvKey::OauthWhoamiUrl,
            SettingName::OauthClientId => RoverEnvKey::OauthClientId,
            _ => panic!("not an OAuth setting"),
        }
    }

    /// Resolves one OAuth setting through the stored tiers, printing its
    /// override notice only when `notice` is set - i.e. only when the calling
    /// command actually sends a request to that endpoint (FR60). `Ok(None)`
    /// means nothing overrode it; `OauthConfig::new` applies the built-in
    /// default.
    #[cfg(feature = "oauth")]
    fn resolve_oauth_setting(
        &self,
        name: SettingName,
        notice: bool,
    ) -> RoverResult<Option<String>> {
        let flag_or_env = self.oauth_flag_or_env(name);
        let resolved = self.resolve_setting(flag_or_env.clone(), name)?;
        if notice
            && let Some(message) = self.config_override_notice(
                name,
                flag_or_env.as_deref(),
                self.get_env_var(Self::oauth_env_key(name))?.as_deref(),
                resolved.as_deref(),
            )?
        {
            self.print_config_notice(message);
        }
        Ok(resolved)
    }

    /// Builds the OAuth endpoints and client ID through the stored tiers.
    /// Every value is resolved (and so validated) eagerly, but only the
    /// settings in `used` - the ones the running `auth` subcommand actually
    /// contacts - can print an override notice. Eager validation is
    /// deliberate: an invalid stored value for any of the six fails every
    /// `auth` subcommand, even one that never contacts that endpoint, the
    /// same tradeoff an invalid `APOLLO_ROVER_DOWNLOAD_HOST` makes for every
    /// command that builds a client config.
    #[cfg(feature = "oauth")]
    pub(crate) fn get_oauth_config(
        &self,
        used: &[SettingName],
    ) -> RoverResult<command::auth::OauthConfig> {
        let resolve = |name| self.resolve_oauth_setting(name, used.contains(&name));
        let url = |value: Option<String>| {
            value.map(|value| {
                url::Url::parse(&value)
                    .expect("a resolved OAuth URL was already validated as a URL")
            })
        };
        Ok(command::auth::OauthConfig::builder()
            .maybe_authorization_url(url(resolve(SettingName::OauthAuthorizationUrl)?))
            .maybe_token_url(url(resolve(SettingName::OauthTokenUrl)?))
            .maybe_revocation_url(url(resolve(SettingName::OauthRevocationUrl)?))
            .maybe_whoami_url(url(resolve(SettingName::OauthWhoamiUrl)?))
            .maybe_device_authorization_url(url(resolve(SettingName::OauthDeviceAuthorizationUrl)?))
            .maybe_client_id(resolve(SettingName::OauthClientId)?)
            .build())
    }

    pub(crate) async fn get_client_config(&self) -> RoverResult<StudioClientConfig> {
        let override_endpoint =
            self.resolve_setting(self.registry_url.clone(), SettingName::RegistryUrl)?;
        if let Some(message) = self.config_override_notice(
            SettingName::RegistryUrl,
            self.registry_url.as_deref(),
            self.get_env_var(RoverEnvKey::RegistryUrl)?.as_deref(),
            override_endpoint.as_deref(),
        )? {
            self.print_config_notice(message);
        }
        let is_sudo = if let Some(fire_flower) = self.get_env_var(RoverEnvKey::FireFlower)? {
            let fire_flower = fire_flower.to_lowercase();
            fire_flower == "true" || fire_flower == "1"
        } else {
            false
        };
        // Resolved once, here, and threaded into everything below that
        // needs it - each of those used to re-resolve it independently
        // (re-reading the profile and re-deciding the override notice,
        // though the notice's once-per-process gate kept it from actually
        // re-printing).
        let client_timeout = self.get_client_timeout()?;
        let mut config = self.get_rover_config()?;
        if config.override_api_key.is_none() {
            config.override_client_credentials_token = self
                .resolve_client_credentials_token(client_timeout)
                .await?;
        }
        let client_config = StudioClientConfig::new(
            override_endpoint,
            config,
            is_sudo,
            self.get_reqwest_client_builder(client_timeout)?,
            client_timeout.unwrap_or_default(),
        );
        // Downloads should honor the client timeout if resolved (from a flag,
        // env var, or profile) despite having a different default
        let client_config = match client_timeout {
            Some(timeout) => client_config.with_download_timeout(timeout.get_duration()),
            None => client_config,
        };
        // Note: the value's own syntactic validity is still checked eagerly
        // here (via `resolve_setting`), even though the notice below is
        // deferred to the point a download actually happens - an invalid
        // stored `APOLLO_ROVER_DOWNLOAD_HOST` deliberately fails every
        // command that builds a client config, the same way an invalid
        // registry URL already does, rather than only commands that
        // download a plugin.
        let resolved_download_host =
            self.resolve_setting(self.download_host_flag_or_env(), SettingName::DownloadHost)?;
        // The override notice is decided here (so `config_override_notice`'s
        // once-per-process gate is consumed exactly once regardless of
        // whether a download ever happens) but not printed here - printing
        // is deferred to `StudioClientConfig::print_download_host_notice_once`,
        // called only at the point a plugin download actually starts
        // (spec.md:403/FR60: a command that downloads nothing prints
        // nothing).
        let download_host_notice = self.config_override_notice(
            SettingName::DownloadHost,
            self.download_host_flag_or_env().as_deref(),
            self.get_env_var(RoverEnvKey::RoverDownloadHost)?.as_deref(),
            resolved_download_host.as_deref(),
        )?;
        let client_config = match resolved_download_host {
            Some(download_host) => client_config.with_download_host(download_host),
            None => client_config,
        };
        Ok(match download_host_notice {
            Some(message) => client_config.with_download_host_notice(message),
            None => client_config,
        })
    }

    /// Exchanges `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` for an access token via the
    /// OAuth 2.0 client credentials grant, for CI/machine-to-machine use where the
    /// interactive `rover auth login` flow isn't an option. Returns `Ok(None)` when
    /// neither env var is set, so callers can fall back to a stored profile credential.
    #[cfg(feature = "oauth")]
    async fn resolve_client_credentials_token(
        &self,
        client_timeout: Option<ClientTimeout>,
    ) -> RoverResult<Option<String>> {
        use std::time::Duration;

        use bytes::Bytes;
        use rover_auth::oauth2::{
            Scope,
            client_credentials::{ClientCredentials, ClientCredentialsRequest},
        };
        use rover_http::{Full, ReqwestService, retry::retry_with_attempt_timeout};
        use tower::{ServiceBuilder, ServiceExt};

        // Bounds a single token-endpoint attempt, independent of the overall
        // retry budget below - mirrors `WHOAMI_ATTEMPT_TIMEOUT` in
        // `command::auth::whoami`, which uses the same layering.
        const CLIENT_CREDENTIALS_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

        let client_id = self.get_env_var(RoverEnvKey::ClientId)?;
        let client_secret = self.get_env_var(RoverEnvKey::ClientSecret)?;
        let (client_id, client_secret) = match (client_id, client_secret) {
            (Some(client_id), Some(client_secret)) => (client_id, client_secret),
            (Some(_), None) => {
                return Err(anyhow::anyhow!(
                    "{} is set but {} is not; both are required to authenticate with client credentials",
                    RoverEnvKey::ClientId,
                    RoverEnvKey::ClientSecret
                )
                .into());
            }
            (None, Some(_)) => {
                return Err(anyhow::anyhow!(
                    "{} is set but {} is not; both are required to authenticate with client credentials",
                    RoverEnvKey::ClientSecret,
                    RoverEnvKey::ClientId
                )
                .into());
            }
            (None, None) => return Ok(None),
        };

        // `rover:cli` identifies this as a rover request to Apollo's auth server -
        // the same scope `rover-auth`'s dynamic client registration requests
        // (`crates/rover-auth/src/oauth2/register.rs`), minus the `openid`/`profile`/
        // `email` trio that flow also requests: those exist to authenticate a
        // *person* via an ID token, which doesn't apply here - the client
        // credentials grant authenticates the application itself (already
        // identified by `client_id`, sent via HTTP Basic auth on every request),
        // not a human user.
        // A real request goes to the token endpoint here, so its notice may fire.
        let token_url = self
            .resolve_oauth_setting(SettingName::OauthTokenUrl, true)?
            .map(|value| {
                url::Url::parse(&value)
                    .expect("a resolved OAuth URL was already validated as a URL")
            })
            .unwrap_or_else(|| crate::options::DEFAULT_TOKEN_URL.clone());
        let request = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![Scope::new("rover:cli".to_string())])
            .build()
            .map_err(|e| anyhow::anyhow!("invalid client credentials: {e}"))?;

        let raw_service = ReqwestService::builder()
            .client(self.get_reqwest_client(client_timeout)?)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?;

        // Bound each attempt and retry transient failures (timeouts, connect
        // errors, 5xx/429) - a flaky token endpoint shouldn't fail a CI job's
        // first authenticated request outright. Same layering as the OAuth
        // whoami lookup in `command::auth::whoami`.
        let http_service = ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                client_timeout.unwrap_or_default().get_duration(),
                CLIENT_CREDENTIALS_ATTEMPT_TIMEOUT,
            ))
            .service(raw_service);

        let service: ClientCredentials<_, Full<Bytes>> = ClientCredentials::new(http_service);
        let response = service.oneshot(request).await.map_err(|e| {
            // `anyhow!` builds a new error with no source, so the chain stops here and the
            // cause has to be rendered into the message to survive at all.
            anyhow::anyhow!(
                "failed to exchange client credentials for an access token: {}",
                rover_std::format_error_chain(&e)
            )
        })?;

        Ok(Some(response.access_token.secret().to_string()))
    }

    #[cfg(not(feature = "oauth"))]
    async fn resolve_client_credentials_token(
        &self,
        _client_timeout: Option<ClientTimeout>,
    ) -> RoverResult<Option<String>> {
        Ok(None)
    }

    pub(crate) fn get_install_override_path(&self) -> RoverResult<Option<Utf8PathBuf>> {
        Ok(self.rover_home.clone())
    }

    pub(crate) fn get_git_context(&self) -> RoverResult<GitContext> {
        // constructing GitContext with a set of overrides from --vcs-* flags/env vars
        let override_git_context = GitContext {
            branch: self.vcs_branch.clone(),
            commit: self.vcs_commit.clone(),
            author: self.vcs_author.clone(),
            remote_url: self.vcs_remote_url.clone(),
        };

        let git_context = GitContext::new_with_override(override_git_context);
        tracing::debug!(?git_context);
        Ok(git_context)
    }

    /// `client_timeout` lets a caller that's already resolved the setting
    /// (`get_client_config`) pass it straight through instead of this
    /// re-resolving it (a second profile read and override-notice decision);
    /// pass `None` when there's nothing already resolved to hand in. Only
    /// matters on a cold cache - once `self.client` is populated the value
    /// passed in here is moot.
    pub(crate) fn get_reqwest_client(
        &self,
        client_timeout: Option<ClientTimeout>,
    ) -> RoverResult<Client> {
        if let Some(client) = self.client.borrow() {
            Ok(client.clone())
        } else {
            let client = self.get_reqwest_client_builder(client_timeout)?.build()?;
            let _ = self.client.fill(client);
            self.get_reqwest_client(None)
        }
    }

    /// See `get_reqwest_client` - same `client_timeout` contract.
    pub(crate) fn get_reqwest_client_builder(
        &self,
        client_timeout: Option<ClientTimeout>,
    ) -> RoverResult<ClientBuilder> {
        // return a copy of the underlying client builder if it's already been populated
        if let Some(client_builder) = self.client_builder.borrow() {
            Ok(*client_builder)
        } else {
            // if a request hasn't been made yet, this cell won't be populated yet -
            // resolution (and thus a profile read) only happens on this first call,
            // and only if the caller didn't already resolve it itself
            let client_timeout = match client_timeout {
                Some(client_timeout) => client_timeout,
                None => self.get_client_timeout()?.unwrap_or_default(),
            };
            self.client_builder
                .fill(
                    ClientBuilder::new()
                        .accept_invalid_certs(self.accept_invalid_certs)
                        .accept_invalid_hostnames(self.accept_invalid_hostnames)
                        .with_timeout(client_timeout.get_duration()),
                )
                .ok();
            self.get_reqwest_client_builder(None)
        }
    }

    /// The raw, clap-merged `--checks-timeout`/`APOLLO_CHECKS_TIMEOUT_SECONDS`
    /// value, before the stored tiers apply. See `registry_url_flag_or_env`.
    pub(crate) fn checks_timeout_flag_or_env(&self) -> Option<String> {
        self.checks_timeout.map(|value| value.to_string())
    }

    pub(crate) fn get_checks_timeout_seconds(&self) -> RoverResult<u64> {
        let resolved = self.resolve_setting(
            self.checks_timeout_flag_or_env(),
            SettingName::ChecksTimeoutSeconds,
        )?;
        if let Some(message) = self.config_override_notice(
            SettingName::ChecksTimeoutSeconds,
            self.checks_timeout_flag_or_env().as_deref(),
            self.get_env_var(RoverEnvKey::ChecksTimeoutSeconds)?
                .as_deref(),
            resolved.as_deref(),
        )? {
            self.print_config_notice(message);
        }
        let resolved = resolved.or_else(|| SettingName::ChecksTimeoutSeconds.builtin_default());
        Ok(resolved
            .expect("APOLLO_CHECKS_TIMEOUT_SECONDS always has a builtin default")
            .parse()
            .expect(
                "a resolved APOLLO_CHECKS_TIMEOUT_SECONDS value was already validated as a \
                whole number of seconds",
            ))
    }

    /// The raw, clap-merged `--client-timeout`/`APOLLO_CLIENT_TIMEOUT` value,
    /// before the stored tiers apply. See `registry_url_flag_or_env`.
    pub(crate) fn client_timeout_flag_or_env(&self) -> Option<String> {
        self.client_timeout.map(|value| value.to_string())
    }

    /// Resolves `APOLLO_CLIENT_TIMEOUT`'s effective value (FR1), printing the
    /// FR59(a) override notice when a real env var overrides an explicit
    /// profile (this setting isn't a network destination, so FR59(b)'s
    /// "profile sets a non-default value" case never fires for it - matches
    /// `get_checks_timeout_seconds`). `Ok(None)` means nothing resolved from
    /// a flag, env var, profile, or the project file - callers that need a concrete value fall
    /// back to `ClientTimeout::default()` themselves; callers like
    /// `get_client_config` that also decide whether to extend this timeout
    /// to plugin downloads need to tell "nothing resolved" apart from "the
    /// resolved value happens to be the default", so this returns the
    /// `Option` rather than pre-applying the fallback the way
    /// `get_checks_timeout_seconds` does.
    pub(crate) fn get_client_timeout(&self) -> RoverResult<Option<ClientTimeout>> {
        let resolved = self.resolve_setting(
            self.client_timeout_flag_or_env(),
            SettingName::ClientTimeout,
        )?;
        if let Some(message) = self.config_override_notice(
            SettingName::ClientTimeout,
            self.client_timeout_flag_or_env().as_deref(),
            self.get_env_var(RoverEnvKey::ClientTimeout)?.as_deref(),
            resolved.as_deref(),
        )? {
            self.print_config_notice(message);
        }
        Ok(resolved.map(|value| {
            ClientTimeout::new(value.parse().expect(
                "a resolved APOLLO_CLIENT_TIMEOUT value was already validated as a whole \
                number of seconds",
            ))
        }))
    }

    /// Resolves `APOLLO_GRAPH_REF`'s effective value (FR6): the real
    /// environment variable, falling through to the stored tiers (an
    /// explicit profile, the project file, the default profile), falling
    /// through again to `None` (this setting has no builtin default, per
    /// FR1). There's no flag for this setting - FR6
    /// deliberately keeps `--graph-ref` as a separate, unrelated flag - so
    /// the real env var is itself the highest tier, unlike every other
    /// setting in this slice, which has a flag above its own env var.
    ///
    /// Called unconditionally for every `rover dev` invocation, so an
    /// invalid stored value fails the command even when `--graph-ref`
    /// already resolved a `RemoteRouterConfig` and the profile value would
    /// never actually be read (`RunRouter::auth_env`'s `Some(remote_config)`
    /// branch wins first). This is a deliberate eager-validation tradeoff,
    /// consistent with how an invalid `APOLLO_ROVER_DOWNLOAD_HOST` also
    /// fails every command rather than only ones that download a plugin -
    /// not deferred to the point of actual use.
    pub(crate) fn resolve_graph_ref_setting(&self) -> RoverResult<Option<String>> {
        let raw_env = self.get_env_var(RoverEnvKey::GraphRef)?;
        let resolved = self.resolve_setting(raw_env.clone(), SettingName::GraphRef)?;
        if let Some(message) = self.config_override_notice(
            SettingName::GraphRef,
            raw_env.as_deref(),
            raw_env.as_deref(),
            resolved.as_deref(),
        )? {
            self.print_config_notice(message);
        }
        Ok(resolved)
    }

    pub(crate) fn get_env_var(&self, key: RoverEnvKey) -> io::Result<Option<String>> {
        Ok(if let Some(env_store) = self.env_store.borrow() {
            env_store.get(key)
        } else {
            let env_store = RoverEnv::new()?;
            let val = env_store.get(key);
            self.env_store
                .fill(env_store)
                .expect("Could not overwrite the existing environment variable store");
            val
        })
    }

    #[cfg(test)]
    pub(crate) fn insert_env_var(&mut self, key: RoverEnvKey, value: &str) -> io::Result<()> {
        if let Some(env_store) = self.env_store.borrow_mut() {
            env_store.insert(key, value)
        } else {
            let mut env_store = RoverEnv::new()?;
            env_store.insert(key, value);
            self.env_store
                .fill(env_store)
                .expect("Could not overwrite the existing environment variable store");
        };
        Ok(())
    }
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Initialize a federated graph in your current directory
    Init(command::Init),

    /// API Key Related Commands
    #[clap(name = "api-key")]
    ApiKeys(command::ApiKeys),

    /// Authentication commands
    #[cfg(feature = "oauth")]
    Auth(command::Auth),

    #[cfg(feature = "composition-js")]
    Connector(command::Connector),

    /// Generate shell completion scripts
    Completion(command::Completion),

    /// Configuration profile commands
    Config(command::Config),

    /// Contract configuration commands
    Contract(command::Contract),

    /// Schema inspection commands
    Schema(command::Schema),

    /// Run a supergraph locally to develop and test subgraph changes
    ///
    /// ⚠️ Do not run this command in production!
    /// ⚠️ It is intended for local development.
    ///
    /// You can navigate to the supergraph endpoint in your browser
    /// to execute operations and see query plans using Apollo Sandbox.
    Dev(Box<command::Dev>),

    /// Supergraph schema commands
    Supergraph(command::Supergraph),

    /// Graph API schema commands
    Graph(command::Graph),

    /// Commands for working with templates
    Template(command::Template),

    /// Readme commands
    Readme(command::Readme),

    /// Subgraph schema commands
    Subgraph(command::Subgraph),

    /// Interact with Rover's documentation
    Docs(command::Docs),

    /// Commands related to updating rover
    Update(command::Update),

    /// Commands for persisted queries
    #[command(visible_alias = "pq")]
    PersistedQueries(command::PersistedQueries),

    /// Installs Rover
    Install(command::Install),

    /// Plugin management commands
    Plugin(command::Plugins),

    /// Get system information
    #[command(hide = true)]
    Info(command::Info),

    /// Explain error codes
    Explain(command::Explain),

    /// Commands for fetching offline licenses
    License(command::License),

    /// Start the language server
    #[cfg(feature = "composition-js")]
    #[clap(hide = true)]
    Lsp(command::Lsp),

    /// Client workflow commands
    Client(command::Client),

    /// Graph artifact commands
    GraphArtifact(command::GraphArtifact),
}

#[derive(Default, ValueEnum, Debug, Serialize, Clone, Copy, Eq, PartialEq)]
pub enum RoverOutputFormatKind {
    #[default]
    Plain,
    Json,
}

impl Display for RoverOutputFormatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoverOutputFormatKind::Plain => write!(f, "plain"),
            RoverOutputFormatKind::Json => write!(f, "json"),
        }
    }
}

#[derive(ValueEnum, Debug, Serialize, Clone, Copy, Eq, PartialEq)]
pub enum RoverOutputKind {
    RoverOutput,
    RoverError,
}

/// The FR84 "which {reason}" tail of a stored setting's validation-failure
/// message. Deliberately doesn't reuse `SettingValueError`'s own `Display`
/// impl, which is written for `rover config set`'s write-time framing
/// ("`{input}` isn't a valid ...") rather than this read-time one ("... is
/// set to `{value}`, which isn't a valid ...").
const fn describe_invalid_value(error: &SettingValueError) -> &'static str {
    match error {
        SettingValueError::InvalidUrl { .. } => {
            "isn't a valid URL. URLs must include a scheme, for example \
            `https://registry.example.com`."
        }
        SettingValueError::UnsupportedUrlScheme { .. } => {
            "isn't a valid URL. Rover only accepts `http`/`https` URLs for this setting."
        }
        SettingValueError::InvalidBool { .. } => "isn't a valid boolean. Use `true` or `false`.",
        SettingValueError::InvalidWholeSeconds { .. } => "isn't a whole number of seconds.",
        SettingValueError::InvalidGraphRef { .. } => {
            "isn't a valid graph ref. Graph refs must be in the format `<NAME>` or \
            `<NAME>@<VARIANT>`, where `<NAME>` must start with a letter and can otherwise \
            only contain letters, numbers, or the characters `-` or `_`, and must be 64 \
            characters or less; `<VARIANT>` must be 63 characters or less."
        }
        SettingValueError::NotAScalar { .. } => "isn't a single value.",
        SettingValueError::NoValue => "has no value.",
    }
}

/// `setting`, the project file's entry for `name`, validated against
/// `name`'s type, with FR84's project-file text when it fails - quoting the
/// key as the file spells it, so a lowercase alias is named the way its
/// author wrote it.
fn validate_project_value(
    name: SettingName,
    setting: &crate::options::ProjectSetting,
) -> RoverResult<String> {
    let validated = match &setting.value {
        ProjectSettingValue::Scalar(raw) => name.setting_type().validate(raw.clone()),
        ProjectSettingValue::NotAScalar(rendered) => Err(SettingValueError::NotAScalar {
            input: rendered.clone(),
        }),
        ProjectSettingValue::Null => Err(SettingValueError::NoValue),
    };
    validated.map_err(|error| {
        let message = match &error {
            SettingValueError::NoValue => format!(
                "`{PROJECT_FILE}` sets `{key}` with no value. Give it a value, or remove the key.",
                key = setting.key,
            ),
            _ => format!(
                "`{PROJECT_FILE}` sets `{key}` to `{raw}`, which {reason}{correction}",
                key = setting.key,
                raw = error.input(),
                reason = describe_invalid_value(&error),
                correction = project_value_correction(&error, name),
            ),
        };
        // See `validate_profile_value` for why the error is kept typed
        // beneath the message.
        RoverError::new(anyhow::Error::new(error).context(message))
    })
}

/// What FR84's project-file text adds after `describe_invalid_value`'s
/// reason, so it always names a concrete correction. There's no `rover
/// config set` to suggest - the project file is hand-authored (FR71) - so a
/// reason that doesn't already say how to fix the value gets an example,
/// fitted to the setting's type when the value wasn't a single value at all.
const fn project_value_correction(error: &SettingValueError, name: SettingName) -> &'static str {
    match error {
        SettingValueError::InvalidWholeSeconds { .. } => {
            " Use a whole number of seconds, for example `300`."
        }
        SettingValueError::NotAScalar { .. } => match name.setting_type() {
            SettingType::Url => " Use a single URL, for example `https://registry.example.com`.",
            SettingType::Bool => " Use `true` or `false`.",
            SettingType::WholeSeconds => " Use a whole number of seconds, for example `300`.",
            SettingType::GraphRef => " Use a single graph ref, for example `my-graph@current`.",
            SettingType::String => " Use a single string.",
        },
        SettingValueError::InvalidUrl { .. }
        | SettingValueError::UnsupportedUrlScheme { .. }
        | SettingValueError::InvalidBool { .. }
        | SettingValueError::InvalidGraphRef { .. }
        | SettingValueError::NoValue => "",
    }
}

/// The FR84 "Run `rover config set {name} {placeholder} ...`" suggestion's
/// argument placeholder - `<value>` for most settings, but a setting whose
/// spec.md example names a more specific placeholder (`APOLLO_CHECKS_
/// TIMEOUT_SECONDS`'s `<seconds>`) uses that instead.
const fn value_placeholder(error: &SettingValueError) -> &'static str {
    match error {
        SettingValueError::InvalidWholeSeconds { .. } => "<seconds>",
        SettingValueError::InvalidUrl { .. }
        | SettingValueError::UnsupportedUrlScheme { .. }
        | SettingValueError::InvalidBool { .. }
        | SettingValueError::InvalidGraphRef { .. }
        | SettingValueError::NotAScalar { .. }
        | SettingValueError::NoValue => "<value>",
    }
}

/// Both manifest levels for a command run from `cwd`: the plugin system's
/// one discovery rule (FR78), with `rover_home` (`--rover-home`/`APOLLO_HOME`)
/// and `home` placing the user level. A working directory that's gone or
/// isn't UTF-8 (`None`) can't be inside a project Rover can name, so only the
/// user level is found.
fn manifest_dirs_for(
    cwd: Option<&camino::Utf8Path>,
    rover_home: Option<&camino::Utf8Path>,
    home: Option<&camino::Utf8Path>,
) -> crate::plugin::discovery::ManifestDirs {
    use crate::plugin::discovery::{ManifestDirs, global_dir};

    match cwd {
        Some(cwd) => ManifestDirs::discover(cwd, rover_home, home),
        None => ManifestDirs {
            global: global_dir(rover_home, home),
            project: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;
    use clap::Parser;
    use speculoos::prelude::*;

    use super::Rover;
    use crate::{
        PKG_NAME, RoverErrorCode,
        options::{ProjectSettings, SettingName},
        plugin::{discovery::ManifestDirs, manifest::MANIFEST_FILE},
        utils::{client::ClientTimeout, env::RoverEnvKey},
    };

    #[test]
    fn checks_timeout_defaults_to_five_minutes() {
        // Wrapped in `temp_env` too, even though it doesn't set anything -
        // `temp_env`'s lock only serializes against *other* `temp_env` calls,
        // and a bare `std::env::var` read here would otherwise race the
        // process-global env var set by the sibling tests below.
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_checks_timeout_seconds().unwrap()).is_equal_to(300);
    }

    #[test]
    fn checks_timeout_flag_wins_over_env_var() {
        let rover = temp_env::with_var("APOLLO_CHECKS_TIMEOUT_SECONDS", Some("999"), || {
            Rover::parse_from([PKG_NAME, "config", "list", "--checks-timeout", "42"])
        });
        assert_that!(rover.get_checks_timeout_seconds().unwrap()).is_equal_to(42);
    }

    #[test]
    fn checks_timeout_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_CHECKS_TIMEOUT_SECONDS", Some("99"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_checks_timeout_seconds().unwrap()).is_equal_to(99);
    }

    #[test]
    fn client_timeout_defaults_to_thirty_seconds() {
        // See `checks_timeout_defaults_to_five_minutes` for why this is
        // wrapped in `temp_env` even though it doesn't set anything.
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_client_timeout().unwrap()).is_none();
    }

    #[test]
    fn client_timeout_flag_wins_over_env_var() {
        let rover = temp_env::with_var("APOLLO_CLIENT_TIMEOUT", Some("999"), || {
            Rover::parse_from([PKG_NAME, "config", "list", "--client-timeout", "42"])
        });
        assert_that!(rover.get_client_timeout().unwrap()).is_equal_to(Some(ClientTimeout::new(42)));
    }

    #[test]
    fn client_timeout_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_CLIENT_TIMEOUT", Some("99"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_client_timeout().unwrap()).is_equal_to(Some(ClientTimeout::new(99)));
    }

    #[tokio::test]
    async fn download_host_flag_is_threaded_into_client_config() {
        let rover = temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            Rover::parse_from([
                PKG_NAME,
                "config",
                "list",
                "--download-host",
                "https://mirror.example.com",
            ])
        });
        let client_config = rover.get_client_config().await.unwrap();
        assert_that!(client_config.download_host().as_deref())
            .is_equal_to(Some("https://mirror.example.com"));
    }

    #[tokio::test]
    async fn download_host_env_var_is_threaded_into_client_config() {
        let rover = temp_env::with_var(
            "APOLLO_ROVER_DOWNLOAD_HOST",
            Some("https://env-mirror.example.com"),
            || Rover::parse_from([PKG_NAME, "config", "list"]),
        );
        let client_config = rover.get_client_config().await.unwrap();
        assert_that!(client_config.download_host().as_deref())
            .is_equal_to(Some("https://env-mirror.example.com"));
    }

    #[tokio::test]
    async fn download_host_is_none_when_neither_flag_nor_env_is_set() {
        let rover = temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        let client_config = rover.get_client_config().await.unwrap();
        assert_that!(client_config.download_host().as_deref()).is_equal_to(None);
    }

    #[tokio::test]
    async fn download_host_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_ROVER_DOWNLOAD_HOST",
            "https://profile-mirror.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.download_host().as_deref())
            .is_equal_to(Some("https://profile-mirror.example.com"));
    }

    #[tokio::test]
    async fn download_host_flag_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_ROVER_DOWNLOAD_HOST",
            "https://profile-mirror.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--download-host",
                "https://flag-mirror.example.com",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.download_host().as_deref())
            .is_equal_to(Some("https://flag-mirror.example.com"));
    }

    // FR43/FR84 (and spec.md:391, the equivalent checks-timeout criterion) -
    // :403 covers the override *notice*, not an invalid stored value.
    #[tokio::test]
    async fn an_invalid_profile_download_host_fails_the_command() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_ROVER_DOWNLOAD_HOST",
            "mirror.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let error = rover
            .get_client_config()
            .await
            .expect_err("expected an invalid stored download host to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_ROVER_DOWNLOAD_HOST` in profile `staging` is set to \
            `mirror.example.com`, which isn't a valid URL. URLs must include a scheme, for \
            example `https://registry.example.com`. Run `rover config set \
            APOLLO_ROVER_DOWNLOAD_HOST <value> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // FR64: profile (explicit or default) supplies a non-default value for
    // a network-destination setting - download-host is one (FR1's "Net"
    // column), unlike checks-timeout.
    #[tokio::test]
    async fn download_host_profile_value_notices_through_the_real_call_path() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_ROVER_DOWNLOAD_HOST",
            "https://profile-mirror.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let rover = with_notice_env_locked(&["APOLLO_ROVER_DOWNLOAD_HOST"], || {
            temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            })
        });

        rover.get_client_config().await.unwrap();

        // The gate `get_client_config`'s own notice decision already
        // consumed means a direct call for the same setting now returns
        // `None` - proving the real call path decided the notice.
        let message = rover.config_override_notice(
            SettingName::DownloadHost,
            None,
            None,
            Some("https://profile-mirror.example.com"),
        );
        assert_that!(message.unwrap()).is_none();
    }

    // `APOLLO_TEMPLATES_API` has no flag on `Rover` itself - its flag is
    // scoped to `template`/`init` (FR1), living on `TemplatesApiOpt`
    // instead - so these test `resolve_templates_api` directly with the
    // same call `command::template::Template::run` makes, rather than
    // through `Rover::parse_from` (which has no `--templates-api` to
    // parse).
    #[test]
    fn templates_api_profile_setting_applies_when_no_explicit_value_is_set() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TEMPLATES_API",
            "https://templates.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        let resolved = rover.resolve_templates_api(None).unwrap();

        assert_that!(resolved).is_equal_to(Some("https://templates.example.com".to_string()));
    }

    #[test]
    fn templates_api_explicit_value_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TEMPLATES_API",
            "https://profile-templates.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        let resolved = rover
            .resolve_templates_api(Some("https://flag-templates.example.com".to_string()))
            .unwrap();

        assert_that!(resolved).is_equal_to(Some("https://flag-templates.example.com".to_string()));
    }

    #[test]
    fn an_invalid_profile_templates_api_fails_the_command() {
        let home =
            config_home_with_setting("staging", "APOLLO_TEMPLATES_API", "templates.example.com");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        let error = rover
            .resolve_templates_api(None)
            .expect_err("expected an invalid stored templates API URL to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_TEMPLATES_API` in profile `staging` is set to \
            `templates.example.com`, which isn't a valid URL. URLs must include a scheme, for \
            example `https://registry.example.com`. Run `rover config set APOLLO_TEMPLATES_API \
            <value> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // `APOLLO_TEMPLATES_API` is a network-destination setting (FR1's "Net"
    // column), so a profile value taking effect must print the same FR64
    // notice every other such setting's resolution does.
    #[test]
    fn templates_api_profile_value_notices_through_the_real_call_path() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TEMPLATES_API",
            "https://profile-templates.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let rover = with_notice_env_locked(&["APOLLO_TEMPLATES_API"], || {
            temp_env::with_var_unset("APOLLO_TEMPLATES_API", || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            })
        });

        rover.resolve_templates_api(None).unwrap();

        // The gate `resolve_templates_api`'s own notice decision already
        // consumed means a direct call for the same setting now returns
        // `None` - proving the real call path decided the notice.
        let message = rover.config_override_notice(
            SettingName::TemplatesApi,
            None,
            None,
            Some("https://profile-templates.example.com"),
        );
        assert_that!(message.unwrap()).is_none();
    }

    // `config_override_notice_is_suppressed_by_the_flag` already proves
    // `--no-config-notices` suppresses the notice mechanism generically
    // (it doesn't care which setting it's deciding for). This pins down
    // the other half for this setting specifically: suppressing the
    // notice must not also break `resolve_templates_api`'s actual value
    // resolution.
    #[test]
    fn templates_api_still_resolves_with_notices_suppressed() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TEMPLATES_API",
            "https://profile-templates.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let resolved = with_notice_env_locked(&["APOLLO_TEMPLATES_API"], || {
            temp_env::with_var_unset("APOLLO_TEMPLATES_API", || {
                let rover = Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "--no-config-notices",
                    "config",
                    "list",
                ]);

                rover.resolve_templates_api(None)
            })
        })
        .unwrap();

        assert_that!(resolved)
            .is_equal_to(Some("https://profile-templates.example.com".to_string()));
    }

    // `APOLLO_GRAPH_REF` has no flag at all (FR6) - the real env var,
    // `insert_env_var`-seeded since there's no clap field to parse it
    // through, is itself the highest tier.
    #[test]
    fn graph_ref_is_none_with_nothing_configured() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "config",
            "list",
        ]);

        assert_that!(rover.resolve_graph_ref_setting()).is_ok_containing(None);
    }

    #[test]
    fn graph_ref_profile_setting_applies_when_no_env_is_set() {
        let home = config_home_with_setting("staging", "APOLLO_GRAPH_REF", "my-graph@staging");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        assert_that!(rover.resolve_graph_ref_setting())
            .is_ok_containing(Some("my-graph@staging".to_string()));
    }

    #[test]
    fn graph_ref_env_var_wins_over_profile_setting() {
        let home = config_home_with_setting("staging", "APOLLO_GRAPH_REF", "profile-graph");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);
        rover
            .insert_env_var(RoverEnvKey::GraphRef, "env-graph")
            .unwrap();

        assert_that!(rover.resolve_graph_ref_setting())
            .is_ok_containing(Some("env-graph".to_string()));
    }

    #[test]
    fn an_invalid_profile_graph_ref_fails_the_command() {
        let home = config_home_with_setting("staging", "APOLLO_GRAPH_REF", "not a graph ref!");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        let error = rover
            .resolve_graph_ref_setting()
            .expect_err("expected an invalid stored graph ref to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_GRAPH_REF` in profile `staging` is set to `not a graph ref!`, \
            which isn't a valid graph ref. Graph refs must be in the format `<NAME>` or \
            `<NAME>@<VARIANT>`, where `<NAME>` must start with a letter and can otherwise \
            only contain letters, numbers, or the characters `-` or `_`, and must be 64 \
            characters or less; `<VARIANT>` must be 63 characters or less. Run `rover config \
            set APOLLO_GRAPH_REF <value> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    #[tokio::test]
    async fn registry_url_flag_wins_over_env_var() {
        let rover = temp_env::with_var(
            "APOLLO_REGISTRY_URL",
            Some("https://env.example.com"),
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "config",
                    "list",
                    "--registry-url",
                    "https://flag.example.com",
                ])
            },
        );
        let client_config = rover.get_client_config().await.unwrap();
        assert_that!(client_config.uri()).is_equal_to(&"https://flag.example.com".to_string());
    }

    #[tokio::test]
    async fn registry_url_env_var_applies_alone() {
        let rover = temp_env::with_var(
            "APOLLO_REGISTRY_URL",
            Some("https://env.example.com"),
            || Rover::parse_from([PKG_NAME, "config", "list"]),
        );
        let client_config = rover.get_client_config().await.unwrap();
        assert_that!(client_config.uri()).is_equal_to(&"https://env.example.com".to_string());
    }

    #[tokio::test]
    async fn registry_url_defaults_to_studio_prod_when_unset() {
        let rover = temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        let client_config = rover.get_client_config().await.unwrap();
        // Mirrors `STUDIO_PROD_API_ENDPOINT` (src/utils/client.rs), which isn't
        // importable here (private to that module).
        assert_that!(client_config.uri())
            .is_equal_to(&"https://api.apollographql.com/graphql".to_string());
    }

    /// Builds a fresh config home and stores one setting on `profile`,
    /// returning the temp dir (keep it alive for the caller's `--config-home`).
    fn config_home_with_setting(profile: &str, key: &str, value: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        houston::Profile::new(profile, &houston_config)
            .set_setting(key, value)
            .unwrap();
        home
    }

    #[tokio::test]
    async fn registry_url_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://profile.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://profile.example.com".to_string());
    }

    #[tokio::test]
    async fn registry_url_flag_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://profile.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--registry-url",
                "https://flag.example.com",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://flag.example.com".to_string());
    }

    #[tokio::test]
    async fn registry_url_env_var_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://profile.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var(
            "APOLLO_REGISTRY_URL",
            Some("https://env.example.com"),
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            },
        );

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://env.example.com".to_string());
    }

    // FR19: a `--profile` that has no stored settings, or doesn't exist on
    // disk at all, isn't an error - every setting falls through.
    #[tokio::test]
    async fn a_missing_profile_falls_through_to_the_builtin_default() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "nonexistent",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri())
            .is_equal_to(&"https://api.apollographql.com/graphql".to_string());
    }

    // FR39/FR83: a *recognized* setting whose stored value fails validation
    // fails the command; it must not fall back to the default.
    #[tokio::test]
    async fn an_invalid_profile_registry_url_fails_the_command() {
        let home =
            config_home_with_setting("staging", "APOLLO_REGISTRY_URL", "registry.example.com");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let error = rover
            .get_client_config()
            .await
            .expect_err("expected an invalid stored registry URL to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_REGISTRY_URL` in profile `staging` is set to \
            `registry.example.com`, which isn't a valid URL. URLs must include a scheme, for \
            example `https://registry.example.com`. Run `rover config set APOLLO_REGISTRY_URL \
            <value> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // FR59(b) is network-destination-only, and checks-timeout isn't one
    // (FR1's blank "Net" column) - a non-default resolved value alone must
    // never notice, unlike APOLLO_REGISTRY_URL's own equivalent case.
    #[test]
    fn checks_timeout_resolved_value_alone_does_not_notice() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CHECKS_TIMEOUT_SECONDS"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(SettingName::ChecksTimeoutSeconds, None, None, Some("600"))
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    // FR59(a) applies to every setting, not just network destinations - an
    // env var silently overriding an explicitly-selected profile's value
    // must still notice for checks-timeout.
    #[test]
    fn checks_timeout_env_overriding_an_explicit_profile_notices() {
        let home = config_home_with_setting("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CHECKS_TIMEOUT_SECONDS"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::ChecksTimeoutSeconds,
                Some("120"),
                Some("120"),
                Some("120"),
            )
        })
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "`APOLLO_CHECKS_TIMEOUT_SECONDS` from the environment overrides the value set in \
            profile `staging`."
                .to_string(),
        );
    }

    // Confirms `get_checks_timeout_seconds` actually reaches the notice
    // system at all (not just that hand-built inputs decide correctly) -
    // the gate it consumes means a direct call for the same setting
    // afterward is always `None`, regardless of what it's called with.
    #[test]
    fn get_checks_timeout_seconds_decides_a_notice_through_the_real_call_path() {
        let home = config_home_with_setting("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CHECKS_TIMEOUT_SECONDS"], || {
            let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            });

            rover.get_checks_timeout_seconds().unwrap();

            rover.config_override_notice(SettingName::ChecksTimeoutSeconds, None, None, Some("600"))
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn checks_timeout_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        assert_that!(rover.get_checks_timeout_seconds()).is_ok_containing(600);
    }

    #[test]
    fn checks_timeout_flag_wins_over_profile_setting() {
        let home = config_home_with_setting("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--checks-timeout",
                "120",
                "config",
                "list",
            ])
        });

        assert_that!(rover.get_checks_timeout_seconds()).is_ok_containing(120);
    }

    #[test]
    fn checks_timeout_falls_back_to_the_builtin_default_with_no_flag_env_or_profile() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "list",
            ])
        });

        assert_that!(rover.get_checks_timeout_seconds()).is_ok_containing(300);
    }

    // FR43/FR84: the exact example text spec.md gives for this setting.
    #[test]
    fn an_invalid_profile_checks_timeout_fails_the_command() {
        let home = config_home_with_setting("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "soon");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let error = rover
            .get_checks_timeout_seconds()
            .expect_err("expected an invalid stored checks timeout to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_CHECKS_TIMEOUT_SECONDS` in profile `staging` is set to `soon`, \
            which isn't a whole number of seconds. Run `rover config set \
            APOLLO_CHECKS_TIMEOUT_SECONDS <seconds> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // FR59(b) is network-destination-only, and client-timeout isn't one
    // (FR1's blank "Net" column) - a non-default resolved value alone must
    // never notice, unlike APOLLO_REGISTRY_URL's own equivalent case.
    #[test]
    fn client_timeout_resolved_value_alone_does_not_notice() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CLIENT_TIMEOUT"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(SettingName::ClientTimeout, None, None, Some("60"))
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    // FR59(a) applies to every setting, not just network destinations - an
    // env var silently overriding an explicitly-selected profile's value
    // must still notice for client-timeout.
    #[test]
    fn client_timeout_env_overriding_an_explicit_profile_notices() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "60");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CLIENT_TIMEOUT"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::ClientTimeout,
                Some("15"),
                Some("15"),
                Some("15"),
            )
        })
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "`APOLLO_CLIENT_TIMEOUT` from the environment overrides the value set in profile \
            `staging`."
                .to_string(),
        );
    }

    // Confirms `get_client_timeout` actually reaches the notice system at
    // all (not just that hand-built inputs decide correctly) - the gate it
    // consumes means a direct call for the same setting afterward is always
    // `None`, regardless of what it's called with.
    #[test]
    fn get_client_timeout_decides_a_notice_through_the_real_call_path() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "60");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_CLIENT_TIMEOUT"], || {
            let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            });

            rover.get_client_timeout().unwrap();

            rover.config_override_notice(SettingName::ClientTimeout, None, None, Some("60"))
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn client_timeout_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "60");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        assert_that!(rover.get_client_timeout()).is_ok_containing(Some(ClientTimeout::new(60)));
    }

    #[test]
    fn client_timeout_flag_wins_over_profile_setting() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "60");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--client-timeout",
                "15",
                "config",
                "list",
            ])
        });

        assert_that!(rover.get_client_timeout()).is_ok_containing(Some(ClientTimeout::new(15)));
    }

    #[test]
    fn client_timeout_falls_back_to_the_builtin_default_with_no_flag_env_or_profile() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "list",
            ])
        });

        let resolved = rover.get_client_timeout().unwrap();

        assert_that!(resolved).is_none();
        assert_that!(resolved.unwrap_or_default().get_duration())
            .is_equal_to(ClientTimeout::default().get_duration());
    }

    // FR43/FR84: the same example-text shape as every other whole-seconds setting.
    #[test]
    fn an_invalid_profile_client_timeout_fails_the_command() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "soon");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let error = rover
            .get_client_timeout()
            .expect_err("expected an invalid stored client timeout to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_CLIENT_TIMEOUT` in profile `staging` is set to `soon`, which \
            isn't a whole number of seconds. Run `rover config set APOLLO_CLIENT_TIMEOUT \
            <seconds> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // Extends the "an explicit value also shortens/lengthens the download
    // timeout" behavior (previously flag/env-only) to a profile-resolved
    // value too - leaving it flag/env-only here would be the same kind of
    // partially-wired bug the review pass on this spec's other settings
    // caught (the download-host notice firing everywhere but plugin
    // installs, templates-api never noticing at all, and so on).
    #[tokio::test]
    async fn client_timeout_profile_value_also_overrides_the_download_timeout() {
        let home = config_home_with_setting("staging", "APOLLO_CLIENT_TIMEOUT", "15");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_CLIENT_TIMEOUT", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        let client_config = rover.get_client_config().await.unwrap();

        assert_that!(client_config.download_timeout().as_secs()).is_equal_to(15);
    }

    #[test]
    fn telemetry_url_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TELEMETRY_URL",
            "https://telemetry.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_TELEMETRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        assert_that!(rover.telemetry_url_override())
            .is_equal_to(Some("https://telemetry.example.com".to_string()));
    }

    #[test]
    fn telemetry_disabled_profile_setting_true_disables_telemetry() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "true");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        assert_that!(rover.is_telemetry_disabled()).is_true();
    }

    #[test]
    fn telemetry_disabled_profile_setting_false_does_not_disable_telemetry() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "false");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        assert_that!(rover.is_telemetry_disabled()).is_false();
    }

    #[test]
    fn telemetry_disabled_flag_wins_over_a_stored_false() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "false");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "--telemetry-disabled",
            "config",
            "list",
        ]);

        assert_that!(rover.is_telemetry_disabled()).is_true();
    }

    // Telemetry resolution is deliberately best-effort (see
    // `telemetry_url_override`'s doc comment): an invalid stored value is
    // logged and ignored rather than failing a command telemetry has
    // nothing to do with.
    #[test]
    fn telemetry_url_override_ignores_an_invalid_profile_setting() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_URL", "not a url");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_var_unset("APOLLO_TELEMETRY_URL", || {
            Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ])
        });

        assert_that!(rover.telemetry_url_override()).is_none();
    }

    // Nothing is ever sent to `APOLLO_TELEMETRY_URL` when telemetry is
    // disabled, so `telemetry_url_override` must not decide (and so consume
    // the once-per-process gate for) a notice about it either. Proven
    // indirectly: if the disabled call had decided the notice, the gate
    // would already be spent and this direct call would return `None`.
    #[test]
    fn telemetry_url_override_does_not_notice_when_telemetry_is_disabled_by_flag() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TELEMETRY_URL",
            "https://telemetry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_TELEMETRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--telemetry-disabled",
                "config",
                "list",
            ]);

            rover.telemetry_url_override();

            rover.config_override_notice(
                SettingName::TelemetryUrl,
                None,
                None,
                Some("https://telemetry.staging.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some();
    }

    #[test]
    fn telemetry_url_override_does_not_notice_when_telemetry_is_disabled_by_env() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_TELEMETRY_URL",
            "https://telemetry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_TELEMETRY_URL"], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            // `RoverEnv::new()` reads nothing from the real environment in
            // test builds, so the env var has to be seeded through the
            // test-only store instead of a real `APOLLO_TELEMETRY_DISABLED`.
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "true")
                .unwrap();

            rover.telemetry_url_override();

            rover.config_override_notice(
                SettingName::TelemetryUrl,
                None,
                None,
                Some("https://telemetry.staging.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some();
    }

    #[test]
    fn telemetry_disabled_ignores_an_invalid_profile_setting() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "not-a-bool");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ]);

        assert_that!(rover.is_telemetry_disabled()).is_false();
    }

    /// Every test below reads `APOLLO_ROVER_NO_CONFIG_NOTICES` (via
    /// `config_notices_suppressed`) and, for the `config_override_notice`
    /// tests, `APOLLO_REGISTRY_URL` too (both as clap's own env fallback
    /// during parsing, and again independently inside the function under
    /// test) - both real env vars, so the whole test body (parse *and* the
    /// call under test) has to run inside one lock, the same way sibling
    /// tests elsewhere in this file guard `APOLLO_REGISTRY_URL`.
    fn with_notice_env_locked<R>(extra_locked: &[&str], body: impl FnOnce() -> R) -> R {
        let mut keys = vec!["APOLLO_ROVER_NO_CONFIG_NOTICES"];
        keys.extend_from_slice(extra_locked);
        temp_env::with_vars_unset(keys, body)
    }

    // FR64: profile (explicit or default) supplies a non-default value for
    // a network-destination setting, nothing overriding it.
    #[test]
    fn config_override_notice_fires_for_a_non_default_profile_network_value() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some("https://registry.staging.example.com"),
            )
        })
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "profile `staging` sets `APOLLO_REGISTRY_URL` to \
            `https://registry.staging.example.com`."
                .to_string(),
        );
    }

    #[cfg(feature = "oauth")]
    const OAUTH_ENV: [&str; 6] = [
        "APOLLO_OAUTH_AUTHORIZATION_URL",
        "APOLLO_OAUTH_TOKEN_URL",
        "APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL",
        "APOLLO_OAUTH_REVOCATION_URL",
        "APOLLO_OAUTH_WHOAMI_URL",
        "APOLLO_OAUTH_CLIENT_ID",
    ];

    #[cfg(feature = "oauth")]
    fn oauth_rover(home: &tempfile::TempDir, extra_args: &[&str]) -> Rover {
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        temp_env::with_vars_unset(OAUTH_ENV, || {
            let mut args = vec![
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
            ];
            args.extend_from_slice(extra_args);
            args.extend(["config", "list"]);
            Rover::parse_from(args)
        })
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_profile_setting_applies_when_no_flag_or_env_is_set() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_OAUTH_TOKEN_URL",
            "https://auth.staging.example.com/token",
        );
        let rover = oauth_rover(&home, &[]);

        let config = rover.get_oauth_config(&[]).unwrap();

        assert_that!(config.token_url.as_str())
            .is_equal_to("https://auth.staging.example.com/token");
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_flag_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_OAUTH_TOKEN_URL",
            "https://auth.staging.example.com/token",
        );
        let rover = oauth_rover(
            &home,
            &["--oauth-token-url", "https://auth.flag.example.com/token"],
        );

        let config = rover.get_oauth_config(&[]).unwrap();

        assert_that!(config.token_url.as_str()).is_equal_to("https://auth.flag.example.com/token");
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_env_var_wins_over_profile_setting() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_OAUTH_TOKEN_URL",
            "https://auth.staging.example.com/token",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = temp_env::with_vars_unset(OAUTH_ENV, || {
            temp_env::with_var(
                "APOLLO_OAUTH_TOKEN_URL",
                Some("https://auth.env.example.com/token"),
                || {
                    Rover::parse_from([
                        PKG_NAME,
                        "--config-home",
                        home_path.as_str(),
                        "--profile",
                        "staging",
                        "config",
                        "list",
                    ])
                },
            )
        });

        let config = rover.get_oauth_config(&[]).unwrap();

        assert_that!(config.token_url.as_str()).is_equal_to("https://auth.env.example.com/token");
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_falls_back_to_the_builtin_defaults_with_nothing_configured() {
        let home = tempfile::tempdir().unwrap();
        let rover = oauth_rover(&home, &[]);

        let config = rover.get_oauth_config(&[]).unwrap();

        assert_that!(config.token_url.as_str())
            .is_equal_to(crate::options::DEFAULT_TOKEN_URL.as_str());
        assert_that!(config.client_id.as_str()).is_equal_to(crate::options::DEFAULT_CLIENT_ID);
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn an_invalid_profile_oauth_url_fails_the_command() {
        let home =
            config_home_with_setting("staging", "APOLLO_OAUTH_TOKEN_URL", "auth.example.com");
        let rover = oauth_rover(&home, &[]);

        let error = rover
            .get_oauth_config(&[])
            .expect_err("expected an invalid stored OAuth URL to fail the command");

        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // FR60: only the endpoints the running subcommand contacts may notice.
    // The unused setting's gate must still be open afterward - its own
    // notice is still decided fresh - while the used one's is consumed.
    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_notices_fire_only_for_the_settings_the_command_uses() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_OAUTH_TOKEN_URL",
            "https://auth.staging.example.com/token",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        houston::Profile::new(
            "staging",
            &houston::Config::new(Some(&home_path), None).unwrap(),
        )
        .set_setting(
            "APOLLO_OAUTH_REVOCATION_URL",
            "https://auth.staging.example.com/revoke",
        )
        .unwrap();

        let (used, unused) = with_notice_env_locked(&OAUTH_ENV, || {
            let rover = oauth_rover(&home, &[]);
            rover
                .get_oauth_config(&[SettingName::OauthTokenUrl])
                .unwrap();
            let used = rover.config_override_notice(
                SettingName::OauthTokenUrl,
                None,
                None,
                Some("https://auth.staging.example.com/token"),
            );
            let unused = rover.config_override_notice(
                SettingName::OauthRevocationUrl,
                None,
                None,
                Some("https://auth.staging.example.com/revoke"),
            );
            (used, unused)
        });

        assert_that!(used.unwrap()).is_none();
        assert_that!(unused.unwrap()).is_equal_to(Some(
            "profile `staging` sets `APOLLO_OAUTH_REVOCATION_URL` to \
            `https://auth.staging.example.com/revoke`."
                .to_string(),
        ));
    }

    #[test]
    fn config_override_notice_is_silent_when_the_profile_value_equals_the_builtin_default() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some(&SettingName::RegistryUrl.builtin_default().unwrap()),
            )
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    // FR67: the environment overrides an explicitly selected profile's own
    // non-default network value - one combined notice, not two.
    #[test]
    fn config_override_notice_combines_both_cases_when_both_apply() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = temp_env::with_vars(
            [
                ("APOLLO_REGISTRY_URL", Some("https://env.example.com")),
                ("APOLLO_ROVER_NO_CONFIG_NOTICES", None),
            ],
            || {
                let mut rover = Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ]);
                rover
                    .insert_env_var(RoverEnvKey::RegistryUrl, "https://env.example.com")
                    .unwrap();
                rover.config_override_notice(
                    SettingName::RegistryUrl,
                    Some("https://env.example.com"),
                    Some("https://env.example.com"),
                    Some("https://env.example.com"),
                )
            },
        )
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "`APOLLO_REGISTRY_URL` from the environment is set to `https://env.example.com`, \
            overriding the value set in profile `staging`."
                .to_string(),
        );
        // FR67's combined text never repeats the overridden value itself.
        assert_that!(message).does_not_contain("registry.staging.example.com");
    }

    // FR66: same override, but the setting isn't network-destination, so
    // only the bare "overrides" fact is stated - no value.
    #[test]
    fn config_override_notice_omits_the_value_for_a_non_network_destination_override() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "true");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();
            rover.config_override_notice(
                SettingName::TelemetryDisabled,
                Some("true"),
                Some("true"),
                Some("true"),
            )
        })
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "`APOLLO_TELEMETRY_DISABLED` from the environment overrides the value set in \
            profile `staging`."
                .to_string(),
        );
    }

    #[test]
    fn config_override_notice_is_silent_for_the_default_profile() {
        let home = config_home_with_setting(
            "default",
            "APOLLO_REGISTRY_URL",
            "https://registry.default.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = temp_env::with_vars(
            [
                ("APOLLO_REGISTRY_URL", Some("https://env.example.com")),
                ("APOLLO_ROVER_NO_CONFIG_NOTICES", None),
            ],
            || {
                let mut rover = Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "config",
                    "list",
                ]);
                rover
                    .insert_env_var(RoverEnvKey::RegistryUrl, "https://env.example.com")
                    .unwrap();
                rover.config_override_notice(
                    SettingName::RegistryUrl,
                    Some("https://env.example.com"),
                    Some("https://env.example.com"),
                    Some("https://env.example.com"),
                )
            },
        )
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn config_override_notice_is_silent_when_the_flag_not_the_environment_supplied_it() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--registry-url",
                "https://flag.example.com",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::RegistryUrl,
                Some("https://flag.example.com"),
                None,
                Some("https://flag.example.com"),
            )
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    // Exercises the real wiring (`get_client_config` -> `resolve_setting` ->
    // `config_override_notice`) instead of calling `config_override_notice`
    // with hand-built inputs directly - the kind of test that would have
    // caught the `APOLLO_TELEMETRY_URL` notice firing while telemetry was
    // disabled (see the `telemetry_url_override_does_not_notice_when_*`
    // tests, which cover that wiring gap for telemetry specifically).
    #[tokio::test]
    async fn get_client_config_decides_the_registry_url_notice_through_the_real_call_path() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let rover = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            temp_env::with_var_unset("APOLLO_REGISTRY_URL", || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ])
            })
        });

        rover.get_client_config().await.unwrap();

        // The gate `get_client_config`'s own notice decision already
        // consumed means a direct call for the same setting now returns
        // `None` - proving the real call path decided (and would have
        // printed) the notice, not just that hand-built inputs can.
        let message = rover.config_override_notice(
            SettingName::RegistryUrl,
            None,
            None,
            Some("https://registry.staging.example.com"),
        );
        assert_that!(message.unwrap()).is_none();
    }

    #[test]
    fn config_override_notice_fires_at_most_once_per_setting() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let (first, second) = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            let first = rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some("https://registry.staging.example.com"),
            );
            let second = rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some("https://registry.staging.example.com"),
            );
            (first, second)
        });

        assert_that!(first.unwrap()).is_some();
        assert_that!(second.unwrap()).is_none();
    }

    #[test]
    fn config_override_notice_is_suppressed_by_the_flag() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&["APOLLO_REGISTRY_URL"], || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--no-config-notices",
                "config",
                "list",
            ]);
            rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some("https://registry.staging.example.com"),
            )
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn config_override_notice_is_suppressed_by_the_environment_variable() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = temp_env::with_vars(
            [
                ("APOLLO_REGISTRY_URL", None),
                ("APOLLO_ROVER_NO_CONFIG_NOTICES", Some("true")),
            ],
            || {
                let rover = Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "list",
                ]);
                rover.config_override_notice(
                    SettingName::RegistryUrl,
                    None,
                    None,
                    Some("https://registry.staging.example.com"),
                )
            },
        )
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn telemetry_disabled_override_notice_fires_when_the_environment_overrides_an_explicit_profile()
    {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "false");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&[], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();
            rover.telemetry_disabled_override_notice()
        })
        .unwrap()
        .unwrap();

        assert_that!(message).is_equal_to(
            "`APOLLO_TELEMETRY_DISABLED` from the environment overrides the value set in \
            profile `staging`."
                .to_string(),
        );
    }

    // Regression test: this used to build its `Config` via `get_rover_config`,
    // which creates the config home if it's missing. Since `is_telemetry_
    // disabled` (and so this function) runs on every command via `Session::
    // new`, that broke FR18/FR57's "config show creates nothing" guarantee
    // for any command run against a fresh machine. `tempfile::tempdir()`
    // (used by every other test here) creates the directory immediately,
    // which would mask this - the path here is never actually created.
    #[test]
    fn telemetry_disabled_override_notice_creates_nothing_on_a_fresh_config_home() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = temp_dir.path().join("nonexistent");
        let home_path = camino::Utf8Path::from_path(&home_path).unwrap();

        with_notice_env_locked(&[], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();

            rover.telemetry_disabled_override_notice()
        })
        .unwrap();

        assert_that!(home_path.exists()).is_false();
    }

    // The env var forces `disabled` regardless of its own value, so it only
    // changes anything when the profile wasn't already disabling telemetry.
    #[test]
    fn telemetry_disabled_override_notice_is_silent_when_the_profile_already_disables_it() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "true");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&[], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();
            rover.telemetry_disabled_override_notice()
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn telemetry_disabled_override_notice_is_silent_when_the_flag_won() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "false");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&[], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--telemetry-disabled",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();
            rover.telemetry_disabled_override_notice()
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    #[test]
    fn telemetry_disabled_override_notice_is_silent_without_an_explicit_profile_value() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();

        let message = with_notice_env_locked(&[], || {
            let mut rover = Rover::parse_from([
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "list",
            ]);
            rover
                .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
                .unwrap();
            rover.telemetry_disabled_override_notice()
        })
        .unwrap();

        assert_that!(message).is_none();
    }

    fn rover_for_staging(home_path: &camino::Utf8Path) -> Rover {
        Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "--profile",
            "staging",
            "config",
            "list",
        ])
    }

    // Regression test: this check used to build its `Config` via
    // `get_rover_config`, which creates the config home if it's missing.
    // Since it now runs before every command (including `rover config
    // show`, FR18/FR57), that broke `config show`'s own no-creation
    // guarantee whenever the update check that used to mask this was
    // skipped. `tempfile::tempdir()` (used by every other test here) creates
    // the directory immediately, which would mask this too - the path here
    // is never actually created.
    #[test]
    fn unrecognized_setting_warnings_creates_nothing_on_a_fresh_config_home() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = temp_dir.path().join("nonexistent");
        let home_path = camino::Utf8Path::from_path(&home_path).unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_empty();
        assert_that!(home_path.exists()).is_false();
    }

    #[test]
    fn unrecognized_setting_warnings_is_empty_when_nothing_is_stored() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_empty();
    }

    #[test]
    fn unrecognized_setting_warnings_is_empty_for_only_recognized_settings() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_empty();
    }

    // FR38: a profile carrying a setting this version of Rover doesn't
    // recognize gets one warning naming it, and is otherwise ignored.
    #[test]
    fn unrecognized_setting_warnings_names_an_unrecognized_key() {
        let home = config_home_with_setting("staging", "APOLLO_FUTURE_SETTING", "anything");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_equal_to(vec![
            "Warning: profile `staging` sets `APOLLO_FUTURE_SETTING`, which this version of \
            Rover doesn't recognize. It will be ignored."
                .to_string(),
        ]);
    }

    #[test]
    fn unrecognized_setting_warnings_ignores_recognized_settings_alongside_an_unrecognized_one() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        houston::Profile::new("staging", &houston_config)
            .set_setting("APOLLO_FUTURE_SETTING", "anything")
            .unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_equal_to(vec![
            "Warning: profile `staging` sets `APOLLO_FUTURE_SETTING`, which this version of \
            Rover doesn't recognize. It will be ignored."
                .to_string(),
        ]);
    }

    // FR69 only allows the lowercase alias in the project file - a
    // lowercase-spelled key stored in a profile is a different, unrecognized
    // name at this tier, not the canonical setting it resembles.
    #[test]
    fn unrecognized_setting_warnings_names_a_lowercase_spelled_key() {
        let home = config_home_with_setting("staging", "apollo_registry_url", "anything");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = rover_for_staging(home_path);

        let warnings = rover.unrecognized_setting_warnings(&rover.get_profile_opt());

        assert_that!(warnings).is_equal_to(vec![
            "Warning: profile `staging` sets `apollo_registry_url`, which this version of \
            Rover doesn't recognize. It will be ignored."
                .to_string(),
        ]);
    }

    #[test]
    fn vcs_branch_flag_wins_over_env_var() {
        let rover = temp_env::with_var("APOLLO_VCS_BRANCH", Some("env-branch"), || {
            Rover::parse_from([PKG_NAME, "config", "list", "--vcs-branch", "flag-branch"])
        });
        assert_that!(rover.get_git_context().unwrap().branch)
            .is_equal_to(Some("flag-branch".to_string()));
    }

    #[test]
    fn vcs_branch_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_VCS_BRANCH", Some("env-branch"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_git_context().unwrap().branch)
            .is_equal_to(Some("env-branch".to_string()));
    }

    #[test]
    fn vcs_branch_falls_back_to_git_inference_when_unset() {
        let rover = temp_env::with_var_unset("APOLLO_VCS_BRANCH", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        // `override_git_context.branch` is `None`, so `GitContext::new_with_override`
        // falls through to its own Git inference - just confirming this doesn't
        // panic and the override itself is absent is enough here; the inference
        // logic is `rover-client`'s to test.
        assert_that!(rover.vcs_branch).is_equal_to(None);
    }

    #[test]
    fn vcs_remote_url_flag_wins_over_env_var() {
        let rover = temp_env::with_var(
            "APOLLO_VCS_REMOTE_URL",
            Some("https://env.example.com/repo.git"),
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "config",
                    "list",
                    "--vcs-remote-url",
                    "https://flag.example.com/repo.git",
                ])
            },
        );
        assert_that!(rover.get_git_context().unwrap().remote_url)
            .is_equal_to(Some("https://flag.example.com/repo.git".to_string()));
    }

    #[test]
    fn vcs_remote_url_env_var_applies_alone() {
        let rover = temp_env::with_var(
            "APOLLO_VCS_REMOTE_URL",
            Some("https://env.example.com/repo.git"),
            || Rover::parse_from([PKG_NAME, "config", "list"]),
        );
        assert_that!(rover.get_git_context().unwrap().remote_url)
            .is_equal_to(Some("https://env.example.com/repo.git".to_string()));
    }

    #[test]
    fn vcs_remote_url_falls_back_to_git_inference_when_unset() {
        let rover = temp_env::with_var_unset("APOLLO_VCS_REMOTE_URL", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.vcs_remote_url).is_equal_to(None);
    }

    #[test]
    fn vcs_commit_flag_wins_over_env_var() {
        let rover = temp_env::with_var("APOLLO_VCS_COMMIT", Some("env-sha"), || {
            Rover::parse_from([PKG_NAME, "config", "list", "--vcs-commit", "flag-sha"])
        });
        assert_that!(rover.get_git_context().unwrap().commit)
            .is_equal_to(Some("flag-sha".to_string()));
    }

    #[test]
    fn vcs_commit_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_VCS_COMMIT", Some("env-sha"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_git_context().unwrap().commit)
            .is_equal_to(Some("env-sha".to_string()));
    }

    #[test]
    fn vcs_commit_falls_back_to_git_inference_when_unset() {
        let rover = temp_env::with_var_unset("APOLLO_VCS_COMMIT", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.vcs_commit).is_equal_to(None);
    }

    #[test]
    fn vcs_author_flag_wins_over_env_var() {
        let rover = temp_env::with_var(
            "APOLLO_VCS_AUTHOR",
            Some("Env Author <env@example.com>"),
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "config",
                    "list",
                    "--vcs-author",
                    "Flag Author <flag@example.com>",
                ])
            },
        );
        assert_that!(rover.get_git_context().unwrap().author)
            .is_equal_to(Some("Flag Author <flag@example.com>".to_string()));
    }

    #[test]
    fn vcs_author_env_var_applies_alone() {
        let rover = temp_env::with_var(
            "APOLLO_VCS_AUTHOR",
            Some("Env Author <env@example.com>"),
            || Rover::parse_from([PKG_NAME, "config", "list"]),
        );
        assert_that!(rover.get_git_context().unwrap().author)
            .is_equal_to(Some("Env Author <env@example.com>".to_string()));
    }

    #[test]
    fn vcs_author_falls_back_to_git_inference_when_unset() {
        let rover = temp_env::with_var_unset("APOLLO_VCS_AUTHOR", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.vcs_author).is_equal_to(None);
    }

    #[test]
    fn config_home_flag_wins_over_env_var() {
        let flag_home = tempfile::tempdir().unwrap();
        let flag_home_path = camino::Utf8Path::from_path(flag_home.path()).unwrap();
        let rover = temp_env::with_var("APOLLO_CONFIG_HOME", Some("/env/home"), || {
            Rover::parse_from([
                PKG_NAME,
                "config",
                "list",
                "--config-home",
                flag_home_path.as_str(),
            ])
        });
        let config = rover.get_rover_config().unwrap();
        assert_that!(config.home.as_str()).is_equal_to(flag_home_path.as_str());
    }

    #[test]
    fn rover_home_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_HOME", Some("/env/rover-home"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_that!(rover.get_install_override_path().unwrap())
            .is_equal_to(Some(camino::Utf8PathBuf::from("/env/rover-home")));
    }

    // The project file: its startup checks (FR70, FR72, FR73, FR76), its
    // place in the precedence chain (FR25), and its notices (FR65).

    /// A `rover config list` run (which sends no request of its own) in a
    /// project whose `rover.yaml` is `project`, with a user-level
    /// manifest of `user_level` when given.
    fn rover_in_project(
        project: &str,
        user_level: Option<&str>,
    ) -> (tempfile::TempDir, tempfile::TempDir, Rover) {
        let config_home =
            config_home_with_setting("default", "APOLLO_CHECKS_TIMEOUT_SECONDS", "300");
        let config_home_path = camino::Utf8Path::from_path(config_home.path()).unwrap();
        let tree = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(tree.path().to_path_buf()).unwrap();
        let level = |name: &str, contents: Option<&str>| {
            let dir = root.join(name).join(".rover");
            fs::create_dir_all(&dir).unwrap();
            if let Some(contents) = contents {
                fs::write(dir.join(MANIFEST_FILE), contents).unwrap();
            }
            dir
        };
        let dirs = ManifestDirs {
            project: Some(level("project", Some(project))),
            global: Some(level("home", user_level)),
        };
        let mut rover = Rover::parse_from([
            PKG_NAME,
            "--skip-update-check",
            "--config-home",
            config_home_path.as_str(),
            "config",
            "list",
        ]);
        rover.set_manifest_dirs(dirs);
        (config_home, tree, rover)
    }

    #[tokio::test]
    async fn a_credential_in_the_project_file_fails_the_command() {
        let (_home, _tree, rover) =
            rover_in_project("settings:\n  APOLLO_KEY: service:x:y\n", None);

        let error = rover.execute_command().await.unwrap_err();

        assert_that!(error.message()).is_equal_to(
            "`rover.yaml` sets `APOLLO_KEY` under `settings:`. Credentials can't be \
            stored in a project file. Run `rover auth login`, or set `APOLLO_KEY` in the \
            environment."
                .to_string(),
        );
        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E060);
    }

    #[tokio::test]
    async fn a_setting_spelled_both_ways_fails_the_command() {
        let (_home, _tree, rover) = rover_in_project(
            "settings:\n  APOLLO_REGISTRY_URL: https://a.example.com\n  \
            apollo_registry_url: https://b.example.com\n",
            None,
        );

        let error = rover.execute_command().await.unwrap_err();

        assert_that!(error.message()).is_equal_to(
            "`rover.yaml` sets `APOLLO_REGISTRY_URL` twice, once as \
            `APOLLO_REGISTRY_URL` and once as `apollo_registry_url`. These are the same \
            setting. Remove one."
                .to_string(),
        );
        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E061);
    }

    /// FR89 and AC L394/L427: keys Rover doesn't apply, and a project file
    /// it can't read at all, only warn - and the command still succeeds.
    #[rstest::rstest]
    #[case::unrecognized_key(
        "settings:\n  APOLLO_FUTURE_SETTING: x\n",
        None,
        "Warning: `rover.yaml` sets `APOLLO_FUTURE_SETTING`, which this version of Rover \
        doesn't recognize. It will be ignored."
    )]
    #[case::user_level_settings(
        "plugins: {}\n",
        Some("settings:\n  APOLLO_REGISTRY_URL: https://registry.example.com\n"),
        "Warning: the user-level `rover.yaml` has a `settings:` section, which Rover ignores. Use \
        `rover config set` to store user-level settings in a profile."
    )]
    #[case::not_yaml(
        "settings: \"unterminated\n",
        None,
        "Warning: Rover can't read `rover.yaml`, so none of its settings apply: found \
        unexpected end of stream at line 2 column 1, while scanning a quoted scalar at line 1 \
        column 11"
    )]
    #[tokio::test]
    async fn what_rover_does_not_apply_warns_without_failing_the_command(
        #[case] project: &str,
        #[case] user_level: Option<&str>,
        #[case] warning: &str,
    ) {
        let (_home, _tree, rover) = rover_in_project(project, user_level);

        assert_that!(rover.execute_command().await).is_ok();
        assert_that!(rover.project_settings().unwrap().warnings().to_vec())
            .is_equal_to(vec![warning.to_string()]);
    }

    /// FR29: with no project at all, the real discovery rule finds nothing
    /// to read and nothing to warn about.
    #[test]
    fn no_project_file_reads_as_no_settings() {
        let tree = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dunce::canonicalize(tree.path()).unwrap()).unwrap();
        let cwd = root.join("work").join("graph");
        fs::create_dir_all(&cwd).unwrap();
        let mut rover = Rover::parse_from([PKG_NAME, "config", "list"]);
        rover.set_manifest_dirs(super::manifest_dirs_for(Some(&cwd), None, Some(&root)));

        assert_that!(rover.project_settings().unwrap()).is_equal_to(&ProjectSettings::default());
    }

    /// Discovery from a directory nested inside a project, from one outside
    /// any project, and from a working directory Rover can't name. `home`
    /// is the temp root, so no walk can climb out of the tree.
    #[rstest::rstest]
    #[case::nested_in_a_project(Some("project/graphs/products"), Some("project/.rover"))]
    #[case::outside_any_project(Some("elsewhere"), None)]
    #[case::no_working_directory(None, None)]
    fn manifest_dirs_for_finds_the_nearest_project(
        #[case] cwd: Option<&str>,
        #[case] project: Option<&str>,
    ) {
        let tree = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dunce::canonicalize(tree.path()).unwrap()).unwrap();
        fs::create_dir_all(root.join("project/.rover")).unwrap();
        fs::create_dir_all(root.join("project/graphs/products")).unwrap();
        fs::create_dir_all(root.join("elsewhere")).unwrap();
        let rover_home = root.join("rover-home");
        let cwd = cwd.map(|cwd| root.join(cwd));

        let dirs = super::manifest_dirs_for(cwd.as_deref(), Some(&rover_home), Some(&root));

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(rover_home.join(".rover")),
            project: project.map(|project| root.join(project)),
        });
    }

    #[test]
    fn the_project_file_is_read_once_per_process() {
        let (_home, tree, rover) =
            rover_in_project("settings:\n  APOLLO_FUTURE_SETTING: x\n", None);
        let first = rover.project_settings().unwrap().clone();
        fs::write(
            tree.path().join("project/.rover").join(MANIFEST_FILE),
            "settings:\n  APOLLO_KEY: x\n",
        )
        .unwrap();

        assert_that!(rover.project_settings().unwrap()).is_equal_to(&first);
    }

    /// A run in a temp project whose `rover.yaml` is `project`
    /// (or no project at all), with each `(profile, setting, value)` of
    /// `stored` written to the config home first, and `args` before the
    /// `config list` subcommand. The environment variables in `unset` are
    /// cleared while clap reads them.
    struct Scenario {
        _config_home: tempfile::TempDir,
        _tree: tempfile::TempDir,
        rover: Rover,
    }

    fn scenario(
        project: Option<&str>,
        stored: &[(&str, &str, &str)],
        args: &[&str],
        unset: &[&str],
    ) -> Scenario {
        let config_home = tempfile::tempdir().unwrap();
        let config_home_path = camino::Utf8Path::from_path(config_home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&config_home_path), None).unwrap();
        for (profile, key, value) in stored {
            houston::Profile::new(*profile, &houston_config)
                .set_setting(key, value)
                .unwrap();
        }
        let tree = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(tree.path().to_path_buf()).unwrap();
        let project = project.map(|contents| {
            let dir = root.join("project").join(".rover");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(MANIFEST_FILE), contents).unwrap();
            dir
        });
        let mut full_args = vec![PKG_NAME, "--config-home", config_home_path.as_str()];
        full_args.extend_from_slice(args);
        full_args.extend(["config", "list"]);
        let mut rover = temp_env::with_vars_unset(unset, || Rover::parse_from(full_args));
        rover.set_manifest_dirs(ManifestDirs {
            project,
            global: None,
        });
        Scenario {
            _config_home: config_home,
            _tree: tree,
            rover,
        }
    }

    const REPO_REGISTRY: &str = "settings:\n  APOLLO_REGISTRY_URL: https://repo.example.com\n";

    fn both_profiles() -> [(&'static str, &'static str, &'static str); 2] {
        [
            (
                "staging",
                "APOLLO_REGISTRY_URL",
                "https://staging.example.com",
            ),
            (
                "default",
                "APOLLO_REGISTRY_URL",
                "https://default.example.com",
            ),
        ]
    }

    /// AC: an explicit profile beats the project file, which beats the
    /// default profile - and `--profile default` typed literally is
    /// explicit.
    #[rstest::rstest]
    #[case::explicit_profile(Some(REPO_REGISTRY), &["--profile", "staging"], "https://staging.example.com")]
    #[case::project_file(Some(REPO_REGISTRY), &[], "https://repo.example.com")]
    #[case::default_profile(None, &[], "https://default.example.com")]
    #[case::literal_profile_default(Some(REPO_REGISTRY), &["--profile", "default"], "https://default.example.com")]
    #[tokio::test]
    async fn the_six_tier_chain_orders_the_stored_tiers(
        #[case] project: Option<&str>,
        #[case] args: &[&str],
        #[case] expected: &str,
    ) {
        let scenario = scenario(project, &both_profiles(), args, &["APOLLO_REGISTRY_URL"]);

        let client_config = scenario.rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&expected.to_string());
    }

    #[tokio::test]
    async fn the_environment_beats_the_project_file() {
        let scenario = temp_env::with_var(
            "APOLLO_REGISTRY_URL",
            Some("https://env.example.com"),
            || scenario(Some(REPO_REGISTRY), &[], &[], &[]),
        );

        let client_config = scenario.rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://env.example.com".to_string());
    }

    #[tokio::test]
    async fn an_explicit_profile_without_the_setting_falls_to_the_project_file() {
        let scenario = scenario(
            Some(REPO_REGISTRY),
            &[("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600")],
            &["--profile", "staging"],
            &["APOLLO_REGISTRY_URL"],
        );

        let client_config = scenario.rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://repo.example.com".to_string());
    }

    /// AC: the lowercase alias applies exactly as the canonical spelling.
    #[tokio::test]
    async fn the_lowercase_alias_applies() {
        let scenario = scenario(
            Some("settings:\n  apollo_registry_url: https://repo.example.com\n"),
            &[],
            &[],
            &["APOLLO_REGISTRY_URL"],
        );

        let client_config = scenario.rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&"https://repo.example.com".to_string());
    }

    /// FR84's required project-file text, with the invalid-value code.
    #[tokio::test]
    async fn an_invalid_project_value_fails_with_the_project_text() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_REGISTRY_URL: registry.example.com\n"),
            &[],
            &[],
            &["APOLLO_REGISTRY_URL"],
        );

        let error = scenario.rover.get_client_config().await.unwrap_err();

        assert_that!(error.message()).is_equal_to(
            "`rover.yaml` sets `APOLLO_REGISTRY_URL` to `registry.example.com`, which \
            isn't a valid URL. URLs must include a scheme, for example \
            `https://registry.example.com`."
                .to_string(),
        );
        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E054);
    }

    #[rstest::rstest]
    #[case::lowercase_alias_quoted_as_written(
        "settings:\n  apollo_checks_timeout_seconds: soon\n",
        "`rover.yaml` sets `apollo_checks_timeout_seconds` to `soon`, which isn't a \
        whole number of seconds. Use a whole number of seconds, for example `300`."
    )]
    #[case::not_a_single_value(
        "settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: [300, 600]\n",
        "`rover.yaml` sets `APOLLO_CHECKS_TIMEOUT_SECONDS` to `- 300 - 600`, which \
        isn't a single value. Use a whole number of seconds, for example `300`."
    )]
    #[case::null(
        "settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS:\n",
        "`rover.yaml` sets `APOLLO_CHECKS_TIMEOUT_SECONDS` with no value. Give it a \
        value, or remove the key."
    )]
    fn other_invalid_project_values_name_the_key_as_written(
        #[case] project: &str,
        #[case] expected: &str,
    ) {
        let scenario = scenario(Some(project), &[], &[], &["APOLLO_CHECKS_TIMEOUT_SECONDS"]);

        let error = scenario.rover.get_checks_timeout_seconds().unwrap_err();

        assert_that!(error.message()).is_equal_to(expected.to_string());
        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E054);
    }

    /// The correction for a value that isn't a single value fits the
    /// setting's type.
    #[tokio::test]
    async fn a_url_that_isnt_a_single_value_suggests_a_url() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_REGISTRY_URL: [a, b]\n"),
            &[],
            &[],
            &["APOLLO_REGISTRY_URL"],
        );

        let error = scenario.rover.get_client_config().await.unwrap_err();

        assert_that!(error.message()).is_equal_to(
            "`rover.yaml` sets `APOLLO_REGISTRY_URL` to `- a - b`, which isn't a single \
            value. Use a single URL, for example `https://registry.example.com`."
                .to_string(),
        );
    }

    /// FR83: an invalid winning value never falls through to the next
    /// tier, even one that's valid.
    #[test]
    fn an_invalid_project_value_does_not_fall_back_to_the_default_profile() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: soon\n"),
            &[("default", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600")],
            &[],
            &["APOLLO_CHECKS_TIMEOUT_SECONDS"],
        );

        let error = scenario.rover.get_checks_timeout_seconds().unwrap_err();

        assert_that!(error.message()).is_equal_to(
            "`rover.yaml` sets `APOLLO_CHECKS_TIMEOUT_SECONDS` to `soon`, which isn't a \
            whole number of seconds. Use a whole number of seconds, for example `300`."
                .to_string(),
        );
        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E054);
    }

    /// A tier that loses is never validated.
    #[test]
    fn an_invalid_project_value_beneath_an_explicit_profile_is_not_read() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: soon\n"),
            &[("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600")],
            &["--profile", "staging"],
            &["APOLLO_CHECKS_TIMEOUT_SECONDS"],
        );

        assert_that!(scenario.rover.get_checks_timeout_seconds()).is_ok_containing(600);
    }

    /// FR24: a stored boolean is typed, so `false` in the file means
    /// telemetry stays enabled.
    #[rstest::rstest]
    #[case::typed_true("settings:\n  APOLLO_TELEMETRY_DISABLED: true\n", true)]
    #[case::typed_false("settings:\n  APOLLO_TELEMETRY_DISABLED: false\n", false)]
    fn the_project_file_can_disable_telemetry(#[case] project: &str, #[case] disabled: bool) {
        let scenario = scenario(Some(project), &[], &[], &[]);

        assert_that!(scenario.rover.is_telemetry_disabled()).is_equal_to(disabled);
    }

    #[test]
    fn the_project_file_can_redirect_telemetry() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_TELEMETRY_URL: https://telemetry.example.com\n"),
            &[],
            &[],
            &["APOLLO_TELEMETRY_URL", "APOLLO_TELEMETRY_DISABLED"],
        );

        assert_that!(scenario.rover.telemetry_url_override())
            .is_some()
            .is_equal_to("https://telemetry.example.com".to_string());
    }

    /// FR90: the graph ref a project sets is what's forwarded to a child
    /// process, the same as one from a profile.
    #[test]
    fn the_project_file_supplies_the_graph_ref_to_forward() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_GRAPH_REF: my-graph@staging\n"),
            &[],
            &[],
            &[],
        );

        assert_that!(scenario.rover.resolve_graph_ref_setting())
            .is_ok_containing(Some("my-graph@staging".to_string()));
    }

    /// FR29: with no project file, every setting resolves exactly as the
    /// four-tier chain did - the active profile, whichever it is.
    #[rstest::rstest]
    #[case::explicit(&["--profile", "staging"], "https://staging.example.com")]
    #[case::default(&[], "https://default.example.com")]
    #[tokio::test]
    async fn without_a_project_file_the_active_profile_supplies_the_value(
        #[case] args: &[&str],
        #[case] expected: &str,
    ) {
        let scenario = scenario(None, &both_profiles(), args, &["APOLLO_REGISTRY_URL"]);

        let client_config = scenario.rover.get_client_config().await.unwrap();

        assert_that!(client_config.uri()).is_equal_to(&expected.to_string());
    }

    const MIRROR: &str = "settings:\n  APOLLO_ROVER_DOWNLOAD_HOST: https://mirror.example.com\n";

    /// FR65's required text.
    #[test]
    fn a_project_file_network_value_gets_the_project_file_notice() {
        let scenario = scenario(Some(MIRROR), &[], &[], &["APOLLO_ROVER_DOWNLOAD_HOST"]);

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::DownloadHost,
                None,
                None,
                Some("https://mirror.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some().is_equal_to(
            "`rover.yaml` sets `APOLLO_ROVER_DOWNLOAD_HOST` to \
            `https://mirror.example.com`."
                .to_string(),
        );
    }

    /// The B5 metric: a project file redirecting plugin downloads is
    /// surfaced by exactly one notice, decided through the real call
    /// path - which consumes the once-per-process gate, so a second
    /// decision for the same setting comes back empty.
    #[tokio::test]
    async fn a_project_file_download_host_is_noticed_exactly_once() {
        let scenario = scenario(Some(MIRROR), &[], &[], &["APOLLO_ROVER_DOWNLOAD_HOST"]);
        temp_env::async_with_vars([("APOLLO_ROVER_NO_CONFIG_NOTICES", None::<&str>)], async {
            let client_config = scenario.rover.get_client_config().await.unwrap();

            assert_that!(client_config.download_host_notice_message())
                .is_some()
                .is_equal_to(
                    "`rover.yaml` sets `APOLLO_ROVER_DOWNLOAD_HOST` to \
                        `https://mirror.example.com`.",
                );
            assert_that!(scenario.rover.config_override_notice(
                SettingName::DownloadHost,
                None,
                None,
                Some("https://mirror.example.com"),
            ))
            .is_ok_containing(None);
        })
        .await;
    }

    /// FR63: only the flag or its environment variable can silence it -
    /// and a project-file key trying to is merely unrecognized.
    #[test]
    fn a_project_file_cannot_suppress_its_own_notice() {
        let scenario = scenario(
            Some(
                "settings:\n  APOLLO_ROVER_DOWNLOAD_HOST: https://mirror.example.com\n  \
                APOLLO_ROVER_NO_CONFIG_NOTICES: true\n",
            ),
            &[],
            &[],
            &["APOLLO_ROVER_DOWNLOAD_HOST"],
        );

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::DownloadHost,
                None,
                None,
                Some("https://mirror.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some();
        assert_that!(
            scenario
                .rover
                .project_settings()
                .unwrap()
                .warnings()
                .to_vec()
        )
        .is_equal_to(vec![
            "Warning: `rover.yaml` sets `APOLLO_ROVER_NO_CONFIG_NOTICES`, which \
                this version of Rover doesn't recognize. It will be ignored."
                .to_string(),
        ]);
    }

    #[rstest::rstest]
    #[case::by_the_environment(true, &[])]
    #[case::by_the_flag(false, &["--no-config-notices"])]
    fn the_project_file_notice_is_suppressed(#[case] env: bool, #[case] args: &[&str]) {
        let scenario = scenario(Some(MIRROR), &[], args, &["APOLLO_ROVER_DOWNLOAD_HOST"]);

        let decide = || {
            scenario.rover.config_override_notice(
                SettingName::DownloadHost,
                None,
                None,
                Some("https://mirror.example.com"),
            )
        };
        let message = if env {
            temp_env::with_var("APOLLO_ROVER_NO_CONFIG_NOTICES", Some("true"), decide)
        } else {
            with_notice_env_locked(&[], decide)
        };

        assert_that!(message).is_ok_containing(None);
    }

    #[test]
    fn a_project_file_value_equal_to_the_default_is_silent() {
        let scenario = scenario(
            Some("settings:\n  APOLLO_ROVER_DOWNLOAD_HOST: https://rover.apollo.dev\n"),
            &[],
            &[],
            &["APOLLO_ROVER_DOWNLOAD_HOST"],
        );

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::DownloadHost,
                None,
                None,
                Some("https://rover.apollo.dev"),
            )
        });

        assert_that!(message).is_ok_containing(None);
    }

    /// FR32 and its AC: the explicit profile's own FR64 notice is the only
    /// one, and nothing mentions the project-file value it outranked.
    #[test]
    fn an_explicit_profile_outranking_the_project_file_names_only_the_profile() {
        let scenario = scenario(
            Some(REPO_REGISTRY),
            &both_profiles(),
            &["--profile", "staging"],
            &["APOLLO_REGISTRY_URL"],
        );

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::RegistryUrl,
                None,
                None,
                Some("https://staging.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some().is_equal_to(
            "profile `staging` sets `APOLLO_REGISTRY_URL` to `https://staging.example.com`."
                .to_string(),
        );
    }

    /// The environment overriding the project file is surfaced the way
    /// it is for an explicit profile (FR66/FR67's wording, with the file
    /// named in the profile's place).
    #[rstest::rstest]
    #[case::network_destination(
        REPO_REGISTRY,
        SettingName::RegistryUrl,
        "https://env.example.com",
        "`APOLLO_REGISTRY_URL` from the environment is set to `https://env.example.com`, \
        overriding the value set in `rover.yaml`."
    )]
    #[case::not_a_network_destination(
        "settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: 600\n",
        SettingName::ChecksTimeoutSeconds,
        "900",
        "`APOLLO_CHECKS_TIMEOUT_SECONDS` from the environment overrides the value set in \
        `rover.yaml`."
    )]
    fn the_environment_overriding_the_project_file_is_noticed(
        #[case] project: &str,
        #[case] name: SettingName,
        #[case] env_value: &str,
        #[case] expected: &str,
    ) {
        let scenario = scenario(
            Some(project),
            &both_profiles(),
            &[],
            &["APOLLO_REGISTRY_URL", "APOLLO_CHECKS_TIMEOUT_SECONDS"],
        );

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                name,
                Some(env_value),
                Some(env_value),
                Some(env_value),
            )
        });

        assert_that!(message.unwrap())
            .is_some()
            .is_equal_to(expected.to_string());
    }

    /// The environment overriding an explicit profile is still named as
    /// the profile, even with a project file beneath it.
    #[test]
    fn the_environment_overriding_an_explicit_profile_names_the_profile() {
        let scenario = scenario(
            Some(REPO_REGISTRY),
            &both_profiles(),
            &["--profile", "staging"],
            &["APOLLO_REGISTRY_URL"],
        );

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::RegistryUrl,
                Some("https://env.example.com"),
                Some("https://env.example.com"),
                Some("https://env.example.com"),
            )
        });

        assert_that!(message.unwrap()).is_some().is_equal_to(
            "`APOLLO_REGISTRY_URL` from the environment is set to `https://env.example.com`, \
            overriding the value set in profile `staging`."
                .to_string(),
        );
    }

    /// FR59(a) is about an explicitly selected profile; the default
    /// profile being overridden stays silent, project file or not.
    #[test]
    fn the_environment_overriding_the_default_profile_is_silent() {
        let scenario = scenario(None, &both_profiles(), &[], &["APOLLO_REGISTRY_URL"]);

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::RegistryUrl,
                Some("https://env.example.com"),
                Some("https://env.example.com"),
                Some("https://env.example.com"),
            )
        });

        assert_that!(message).is_ok_containing(None);
    }

    /// A flag, unlike the environment, is never surfaced as an override.
    #[test]
    fn a_flag_overriding_the_project_file_is_silent() {
        let scenario = scenario(Some(REPO_REGISTRY), &[], &[], &["APOLLO_REGISTRY_URL"]);

        let message = with_notice_env_locked(&[], || {
            scenario.rover.config_override_notice(
                SettingName::RegistryUrl,
                Some("https://flag.example.com"),
                None,
                Some("https://flag.example.com"),
            )
        });

        assert_that!(message).is_ok_containing(None);
    }

    #[rstest::rstest]
    #[case::stored_false("settings:\n  APOLLO_TELEMETRY_DISABLED: false\n", true)]
    #[case::already_disabled("settings:\n  APOLLO_TELEMETRY_DISABLED: true\n", false)]
    fn the_environment_disabling_telemetry_over_the_project_file(
        #[case] project: &str,
        #[case] noticed: bool,
    ) {
        let mut scenario = scenario(Some(project), &[], &[], &[]);
        scenario
            .rover
            .insert_env_var(crate::utils::env::RoverEnvKey::TelemetryDisabled, "1")
            .unwrap();

        let message =
            with_notice_env_locked(&[], || scenario.rover.telemetry_disabled_override_notice());

        assert_that!(message.unwrap()).is_equal_to(noticed.then(|| {
            "`APOLLO_TELEMETRY_DISABLED` from the environment overrides the value set in \
            `rover.yaml`."
                .to_string()
        }));
    }
}
