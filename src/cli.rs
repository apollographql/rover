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
        DEFAULT_PROFILE, OutputOpts, ProfileOpt, ProfileSelection, SettingName, SettingValueError,
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

    /// Suppress the notices Rover prints when a profile overrides a
    /// network destination, or when an environment variable overrides a
    /// value an explicitly selected profile also set.
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
        // before running any commands, we check if rover is up to date
        // this only happens once a day automatically
        // we skip this check for the `rover update` commands, since they
        // do their own checks.
        // the check is also skipped if the `--skip-update-check` flag is passed.
        if let Command::Update(_) = &self.command { /* skip check */
        } else if !self.skip_update_check && !crate::utils::skip_all_updates() {
            let config = self.get_rover_config();
            if let Ok(config) = config {
                let _ = version::check_for_update(config, false, self.get_reqwest_client()?).await;
            }
        }

        let profile_opt = self.get_profile_opt();
        for message in self.unrecognized_setting_warnings(&profile_opt) {
            self.print_unrecognized_setting_warning(message);
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
                        self.get_oauth_config(),
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
                    .run(self.get_rover_config()?, self.get_reqwest_client()?)
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
    /// (flag beats env, whichever supplied it) - before the profile tier
    /// applies. `rover config show` (`src/command/config/show/mod.rs`) uses this
    /// to tell a flag/env source apart from a profile one; every other
    /// caller wants `get_client_config`'s fully-resolved value instead.
    pub(crate) fn registry_url_flag_or_env(&self) -> Option<String> {
        self.registry_url.clone()
    }

    /// The raw, clap-merged `--telemetry-url`/`APOLLO_TELEMETRY_URL` value,
    /// before the profile tier applies. See `registry_url_flag_or_env`.
    pub(crate) fn telemetry_url_flag_or_env(&self) -> Option<String> {
        self.telemetry_url.clone()
    }

    /// Whether `--telemetry-disabled` was passed, before the profile tier
    /// applies. See `registry_url_flag_or_env`.
    pub(crate) const fn telemetry_disabled_flag(&self) -> bool {
        self.telemetry_disabled
    }

    /// The raw, clap-merged `--download-host`/`APOLLO_ROVER_DOWNLOAD_HOST`
    /// value, before the profile tier applies. See `registry_url_flag_or_env`.
    pub(crate) fn download_host_flag_or_env(&self) -> Option<String> {
        self.download_host.clone()
    }

    /// The resolved `--telemetry-url`/`APOLLO_TELEMETRY_URL` override, for
    /// `impl Report for Rover` (`src/utils/telemetry.rs`) - a different
    /// module, so it can't reach the private field directly.
    ///
    /// Telemetry reporting is already best-effort and fully isolated from
    /// the invocation's own exit code (see `run`'s `report_thread`), so an
    /// invalid profile-stored value here is logged and skipped rather than
    /// failing a command telemetry has nothing to do with - unlike
    /// `resolve_profile_setting`'s normal contract, this never fails.
    pub(crate) fn telemetry_url_override(&self) -> Option<String> {
        let resolved = if self.telemetry_url.is_some() {
            self.telemetry_url.clone()
        } else {
            match self.resolve_profile_setting(SettingName::TelemetryUrl) {
                Ok(value) => value,
                Err(error) => {
                    tracing::debug!(
                        ?error,
                        "ignoring invalid profile-stored APOLLO_TELEMETRY_URL"
                    );
                    None
                }
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

    /// The resolved `--telemetry-disabled` flag plus its profile tier, for
    /// `impl Report for Rover` (`src/utils/telemetry.rs`).
    /// `APOLLO_TELEMETRY_DISABLED` keeps its own, separate presence-only
    /// check (`RoverEnvKey::TelemetryDisabled`) as its *environment
    /// variable's* parsing (FR21) - this only adds the profile tier
    /// beneath it, where a stored value is a typed boolean (FR24). Errors
    /// are swallowed the same way and for the same reason as
    /// `telemetry_url_override`.
    pub(crate) fn is_telemetry_disabled(&self) -> bool {
        if self.telemetry_disabled {
            return true;
        }
        let resolved = match self.resolve_profile_setting(SettingName::TelemetryDisabled) {
            Ok(Some(value)) => value.eq_ignore_ascii_case("true"),
            Ok(None) => false,
            Err(error) => {
                tracing::debug!(
                    ?error,
                    "ignoring invalid profile-stored APOLLO_TELEMETRY_DISABLED"
                );
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

        let profile = self.get_profile_opt();
        if !profile.selection.is_explicit() {
            return Ok(None);
        }
        let houston_config = self.get_rover_config_read_only()?;
        let profile_raw =
            Profile::new(&profile.profile_name, &houston_config).get_setting(name.as_str())?;
        // The env var disables telemetry on any value it's set to, so it
        // only overrides anything when the profile wasn't already disabling
        // it - a profile that already stores `true` sees no change to notice
        // about.
        Ok(profile_raw
            .filter(|value| !value.eq_ignore_ascii_case("true"))
            .map(|_| {
                format!(
                    "`{name}` from the environment overrides the value set in profile \
                    `{profile_name}`.",
                    profile_name = profile.profile_name,
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

    /// Resolves one setting's effective raw value, adding the profile tier
    /// beneath an already-resolved explicit flag/environment-variable value
    /// (FR25, collapsed to four tiers per FR29 - there's no project file
    /// yet). `Ok(None)` means neither `explicit` nor the active profile
    /// supplied a value, so the caller falls through to its own built-in
    /// default. A stored profile value that fails validation fails the
    /// command outright (FR39/FR83) rather than falling through.
    fn resolve_setting(
        &self,
        explicit: Option<String>,
        name: SettingName,
    ) -> RoverResult<Option<String>> {
        if explicit.is_some() {
            return Ok(explicit);
        }
        self.resolve_profile_setting(name)
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
    /// type. See `resolve_setting` for the tier this fits into. Builds its
    /// own `Config` (creating the config home if it's missing, per
    /// `get_rover_config`'s normal contract) - callers that must not create
    /// anything (`rover config show`, FR18) use
    /// [`Rover::resolve_profile_setting_with`] with their own `Config`
    /// instead.
    pub(crate) fn resolve_profile_setting(&self, name: SettingName) -> RoverResult<Option<String>> {
        let houston_config = self.get_rover_config()?;
        self.resolve_profile_setting_with(&houston_config, name)
    }

    /// Like [`Rover::resolve_profile_setting`], but against a `Config` the
    /// caller already has, rather than building one (and possibly creating
    /// the config home) itself.
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
        let value = name.setting_type().validate(raw).map_err(|error| {
            let (SettingValueError::InvalidUrl { input: raw }
            | SettingValueError::UnsupportedUrlScheme { input: raw }
            | SettingValueError::InvalidBool { input: raw }
            | SettingValueError::InvalidWholeSeconds { input: raw }
            | SettingValueError::InvalidGraphRef { input: raw }) = &error;
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
        Ok(Some(value))
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

        let profile = self.get_profile_opt();
        let profile_is_explicit = profile.selection.is_explicit();
        let houston_config = self.get_rover_config()?;
        let profile_raw =
            Profile::new(&profile.profile_name, &houston_config).get_setting(name.as_str())?;

        let message = if let Some(explicit_value) = explicit {
            // Credits the environment whenever its current value matches
            // what actually resolved, even if the flag (not the env var)
            // supplied that same value - harmless (the message just names
            // the wrong of two identical sources), not corrected here.
            if profile_is_explicit && raw_env == Some(explicit_value) {
                profile_raw.map(|profile_value| {
                    if name.is_network_destination()
                        && Some(profile_value.as_str()) != name.builtin_default().as_deref()
                    {
                        format!(
                            "`{name}` from the environment is set to `{explicit_value}`, \
                            overriding the value set in profile `{profile_name}`.",
                            profile_name = profile.profile_name,
                        )
                    } else {
                        format!(
                            "`{name}` from the environment overrides the value set in profile \
                            `{profile_name}`.",
                            profile_name = profile.profile_name,
                        )
                    }
                })
            } else {
                None
            }
        } else if let Some(value) = resolved {
            (name.is_network_destination() && Some(value) != name.builtin_default().as_deref())
                .then(|| {
                    format!(
                        "profile `{profile_name}` sets `{name}` to `{value}`.",
                        profile_name = profile.profile_name,
                    )
                })
        } else {
            None
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

    #[cfg(feature = "oauth")]
    pub(crate) fn get_oauth_config(&self) -> command::auth::OauthConfig {
        command::auth::OauthConfig::builder()
            .authorization_url(self.oauth_opts.authorization_url.clone())
            .token_url(self.oauth_opts.token_url.clone())
            .revocation_url(self.oauth_opts.revocation_url.clone())
            .whoami_url(self.oauth_opts.whoami_url.clone())
            .device_authorization_url(self.oauth_opts.device_authorization_url.clone())
            .client_id(self.oauth_opts.client_id.clone())
            .build()
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
        let mut config = self.get_rover_config()?;
        if config.override_api_key.is_none() {
            config.override_client_credentials_token =
                self.resolve_client_credentials_token().await?;
        }
        let client_config = StudioClientConfig::new(
            override_endpoint,
            config,
            is_sudo,
            self.get_reqwest_client_builder(),
            self.client_timeout.unwrap_or_default(),
        );
        // Downloads should honor the client timeout if set despite having a different default
        let client_config = match self.client_timeout {
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
    async fn resolve_client_credentials_token(&self) -> RoverResult<Option<String>> {
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
        let request = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(self.oauth_opts.token_url.clone())
            .scopes(vec![Scope::new("rover:cli".to_string())])
            .build()
            .map_err(|e| anyhow::anyhow!("invalid client credentials: {e}"))?;

        let raw_service = ReqwestService::builder()
            .client(self.get_reqwest_client()?)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?;

        // Bound each attempt and retry transient failures (timeouts, connect
        // errors, 5xx/429) - a flaky token endpoint shouldn't fail a CI job's
        // first authenticated request outright. Same layering as the OAuth
        // whoami lookup in `command::auth::whoami`.
        let http_service = ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                self.client_timeout.unwrap_or_default().get_duration(),
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
    async fn resolve_client_credentials_token(&self) -> RoverResult<Option<String>> {
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

    // WARNING: I _think_ this should be an anyhow error (it gets converted to a sputnik error and
    // there's no impl from a rovererror)
    pub(crate) fn get_reqwest_client(&self) -> anyhow::Result<Client> {
        if let Some(client) = self.client.borrow() {
            Ok(client.clone())
        } else {
            let client = self.get_reqwest_client_builder().build()?;
            let _ = self.client.fill(client);
            self.get_reqwest_client()
        }
    }

    pub(crate) fn get_reqwest_client_builder(&self) -> ClientBuilder {
        // return a copy of the underlying client builder if it's already been populated
        if let Some(client_builder) = self.client_builder.borrow() {
            *client_builder
        } else {
            // if a request hasn't been made yet, this cell won't be populated yet
            self.client_builder
                .fill(
                    ClientBuilder::new()
                        .accept_invalid_certs(self.accept_invalid_certs)
                        .accept_invalid_hostnames(self.accept_invalid_hostnames)
                        .with_timeout(self.client_timeout.unwrap_or_default().get_duration()),
                )
                .ok();
            self.get_reqwest_client_builder()
        }
    }

    /// The raw, clap-merged `--checks-timeout`/`APOLLO_CHECKS_TIMEOUT_SECONDS`
    /// value, before the profile tier applies. See `registry_url_flag_or_env`.
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

    /// Resolves `APOLLO_GRAPH_REF`'s effective value (FR6): the real
    /// environment variable, falling through to the active profile's
    /// stored value, falling through again to `None` (this setting has no
    /// builtin default, per FR1). There's no flag for this setting - FR6
    /// deliberately keeps `--graph-ref` as a separate, unrelated flag - so
    /// the real env var is itself the highest tier, unlike every other
    /// setting in this slice, which has a flag above its own env var.
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
        | SettingValueError::InvalidGraphRef { .. } => "<value>",
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use speculoos::prelude::*;

    use super::Rover;
    use crate::{PKG_NAME, options::SettingName, utils::env::RoverEnvKey};

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
            "error[E053]: `APOLLO_GRAPH_REF` in profile `staging` is set to `not a graph ref!`, \
            which isn't a valid graph ref. Graph refs must be in the format `<NAME>` or \
            `<NAME>@<VARIANT>`, where `<NAME>` can only contain letters, numbers, or the \
            characters `-` or `_`, and must be 64 characters or less; `<VARIANT>` must be 64 \
            characters or less. Run `rover config set APOLLO_GRAPH_REF <value> --profile \
            staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E053));
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
}
