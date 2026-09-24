use std::{fmt::Display, io, process};

use camino::Utf8PathBuf;
use clap::{
    Parser, ValueEnum,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use config::Config;
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
    RoverResult,
    command::{self, RoverOutput},
    options::{DEFAULT_PROFILE, OutputOpts, ProfileOpt},
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

    /// Skip checking for newer versions of rover.
    ///
    /// Set the `APOLLO_ROVER_SKIP_UPDATE` environment variable (to `1` or `true`)
    /// to disable all of Rover's auto-updating at once — both this self-update
    /// check and the `supergraph`/`router` plugin auto-updates (`--skip-update`).
    #[arg(long = "skip-update-check", global = true)]
    skip_update_check: bool,

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
}

impl Rover {
    pub async fn run_from_args() -> RoverResult<()> {
        Rover::parse().run().await
    }

    pub async fn run(&self) -> RoverResult<()> {
        timber::init(self.log_level);
        tracing::trace!(command_structure = ?self);
        self.output_opts.set_no_color();

        self.apply_download_host_override();

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

        match &self.command {
            Command::Init(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
            Command::Completion(command) => command.run(),
            Command::Config(command) => {
                command
                    .run(self.get_client_config().await?, &profile_opt)
                    .await
            }
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
                    .run(
                        self.get_client_config().await?,
                        self.get_checks_timeout_seconds()?,
                        &profile_opt,
                    )
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
                        self.get_checks_timeout_seconds()?,
                        &self.output_opts,
                        &profile_opt,
                    )
                    .await
            }
            Command::Template(command) => command.run().await,
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
                        self.get_checks_timeout_seconds()?,
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
        }
    }

    /// `Plugin::get_host()` (`src/command/install/plugin.rs`) reads
    /// `APOLLO_ROVER_DOWNLOAD_HOST` directly, since it has no access to
    /// `Rover`. Exporting the flag's resolved value here - only when the
    /// flag or its env var actually supplied one - lets that existing read
    /// pick up the override without threading it through every
    /// plugin-install call site, mirroring `OutputOpts::set_no_color`'s use
    /// of the same technique for `NO_COLOR`.
    pub(crate) fn apply_download_host_override(&self) {
        if let Some(download_host) = &self.download_host {
            unsafe {
                // SAFETY: called once at startup, before any command runs -
                // single-threaded at this point.
                std::env::set_var("APOLLO_ROVER_DOWNLOAD_HOST", download_host);
            }
        }
    }

    /// The resolved `--telemetry-url`/`APOLLO_TELEMETRY_URL` override, for
    /// `impl Report for Rover` (`src/utils/telemetry.rs`) - a different
    /// module, so it can't reach the private field directly.
    pub(crate) fn telemetry_url_override(&self) -> Option<String> {
        self.telemetry_url.clone()
    }

    /// The resolved `--telemetry-disabled` flag, for `impl Report for Rover`
    /// (`src/utils/telemetry.rs`). `APOLLO_TELEMETRY_DISABLED` keeps its own,
    /// separate presence-only check (`RoverEnvKey::TelemetryDisabled`) -
    /// this is only the flag half of the pair.
    pub(crate) const fn telemetry_disabled_flag(&self) -> bool {
        self.telemetry_disabled
    }

    pub(crate) fn get_rover_config(&self) -> RoverResult<Config> {
        let override_home: Option<Utf8PathBuf> = self
            .get_env_var(RoverEnvKey::ConfigHome)?
            .map(|p| Utf8PathBuf::from(&p));
        let override_api_key = self.get_env_var(RoverEnvKey::Key)?;
        Ok(Config::new(override_home.as_ref(), override_api_key)?)
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
        let override_endpoint = self.registry_url.clone();
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
        Ok(match self.client_timeout {
            Some(timeout) => client_config.with_download_timeout(timeout.get_duration()),
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
        Ok(self
            .get_env_var(RoverEnvKey::Home)?
            .map(|p| Utf8PathBuf::from(&p)))
    }

    pub(crate) fn get_git_context(&self) -> RoverResult<GitContext> {
        // constructing GitContext with a set of overrides from env vars
        let override_git_context = GitContext {
            branch: self.get_env_var(RoverEnvKey::VcsBranch)?,
            commit: self.get_env_var(RoverEnvKey::VcsCommit)?,
            author: self.get_env_var(RoverEnvKey::VcsAuthor)?,
            remote_url: self.get_env_var(RoverEnvKey::VcsRemoteUrl)?,
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

    pub(crate) fn get_checks_timeout_seconds(&self) -> RoverResult<u64> {
        // default to 5 minutes
        Ok(self.checks_timeout.unwrap_or(300))
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Rover;
    use crate::PKG_NAME;

    #[test]
    fn checks_timeout_defaults_to_five_minutes() {
        // Wrapped in `temp_env` too, even though it doesn't set anything -
        // `temp_env`'s lock only serializes against *other* `temp_env` calls,
        // and a bare `std::env::var` read here would otherwise race the
        // process-global env var set by the sibling tests below.
        let rover = temp_env::with_var_unset("APOLLO_CHECKS_TIMEOUT_SECONDS", || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_eq!(rover.get_checks_timeout_seconds().unwrap(), 300);
    }

    #[test]
    fn checks_timeout_flag_wins_over_env_var() {
        let rover = temp_env::with_var("APOLLO_CHECKS_TIMEOUT_SECONDS", Some("999"), || {
            Rover::parse_from([PKG_NAME, "config", "list", "--checks-timeout", "42"])
        });
        assert_eq!(rover.get_checks_timeout_seconds().unwrap(), 42);
    }

    #[test]
    fn checks_timeout_env_var_applies_alone() {
        let rover = temp_env::with_var("APOLLO_CHECKS_TIMEOUT_SECONDS", Some("99"), || {
            Rover::parse_from([PKG_NAME, "config", "list"])
        });
        assert_eq!(rover.get_checks_timeout_seconds().unwrap(), 99);
    }

    #[test]
    fn download_host_flag_is_exported_for_plugin_get_host() {
        temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            let rover = Rover::parse_from([
                PKG_NAME,
                "config",
                "list",
                "--download-host",
                "https://mirror.example.com",
            ]);
            rover.apply_download_host_override();
            assert_eq!(
                std::env::var("APOLLO_ROVER_DOWNLOAD_HOST").unwrap(),
                "https://mirror.example.com"
            );
        });
    }

    #[test]
    fn download_host_is_untouched_when_neither_flag_nor_env_is_set() {
        temp_env::with_var_unset("APOLLO_ROVER_DOWNLOAD_HOST", || {
            let rover = Rover::parse_from([PKG_NAME, "config", "list"]);
            rover.apply_download_host_override();
            assert!(std::env::var("APOLLO_ROVER_DOWNLOAD_HOST").is_err());
        });
    }
}
