use std::fmt::Display;
use std::{io, process};

use camino::Utf8PathBuf;
use clap::builder::styling::{AnsiColor, Effects};
use clap::builder::Styles;
use clap::{Parser, ValueEnum};
use config::Config;
use houston as config;
use lazycell::{AtomicLazyCell, LazyCell};
use reqwest::Client;
use rover_client::shared::GitContext;
use serde::Serialize;
use sputnik::Session;
use timber::Level;

use crate::command::{self, RoverOutput};
use crate::options::OutputOpts;
use crate::utils::client::{ClientBuilder, ClientTimeout, StudioClientConfig};
use crate::utils::env::{RoverEnv, RoverEnvKey};
use crate::utils::stringify::option_from_display;
use crate::utils::version;
use crate::RoverResult;

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
    after_help = "Read the getting started guide by running:

    $ rover docs open start

To begin working with Rover and to authenticate with Apollo Studio,
run the following command:

    $ rover config auth

This will prompt you for an API Key that can be generated in Apollo Studio.

The most common commands from there are:

    - rover graph fetch: Fetch a graph schema from the Apollo graph registry
    - rover graph check: Check for breaking changes in a local graph schema against a graph schema in the Apollo graph
registry
    - rover graph publish: Publish an updated graph schema to the Apollo graph registry

You can open the full documentation for Rover by running:

    $ rover docs open
"
)]
#[command(next_line_help = true)]
pub struct Rover {
    #[clap(subcommand)]
    command: Command,

    /// Specify Rover's log level
    #[arg(long = "log", short = 'l', global = true)]
    #[serde(serialize_with = "option_from_display")]
    log_level: Option<Level>,

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
    #[arg(
        long = "client-timeout",
        global = true,
        default_value_t = ClientTimeout::default()
    )]
    client_timeout: ClientTimeout,

    /// Skip checking for newer versions of rover.
    #[arg(long = "skip-update-check", global = true)]
    skip_update_check: bool,

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
                self.output_opts.handle_output(output)?;

                process::exit(0);
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
        } else if !self.skip_update_check {
            let config = self.get_rover_config();
            if let Ok(config) = config {
                let _ = version::check_for_update(config, false, self.get_reqwest_client()?).await;
            }
        }

        match &self.command {
            #[cfg(feature = "init")]
            Command::Init(command) => command.run(self.get_client_config()?).await,
            Command::Cloud(command) => command.run(self.get_client_config()?).await,
            Command::Config(command) => command.run(self.get_client_config()?).await,
            Command::Contract(command) => command.run(self.get_client_config()?).await,
            Command::Dev(command) => {
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config()?,
                        self.log_level,
                    )
                    .await
            }
            Command::Supergraph(command) => {
                command
                    .run(
                        self.get_install_override_path()?,
                        self.get_client_config()?,
                        self.output_opts.output_file.clone(),
                    )
                    .await
            }
            Command::Docs(command) => command.run(),
            Command::Graph(command) => {
                command
                    .run(
                        self.get_client_config()?,
                        self.get_git_context()?,
                        self.get_checks_timeout_seconds()?,
                        &self.output_opts,
                    )
                    .await
            }
            Command::Template(command) => command.run().await,
            Command::Readme(command) => command.run(self.get_client_config()?).await,
            Command::Subgraph(command) => {
                command
                    .run(
                        self.get_client_config()?,
                        self.get_git_context()?,
                        self.get_checks_timeout_seconds()?,
                        &self.output_opts,
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
                    .do_install(self.get_install_override_path()?, self.get_client_config()?)
                    .await
            }
            Command::Info(command) => command.run(),
            Command::Explain(command) => command.run(),
            Command::PersistedQueries(command) => command.run(self.get_client_config()?).await,
            Command::License(command) => command.run(self.get_client_config()?).await,
            #[cfg(feature = "composition-js")]
            Command::Lsp(command) => command.run(self.get_client_config()?).await,
        }
    }

    pub(crate) fn get_rover_config(&self) -> RoverResult<Config> {
        let override_home: Option<Utf8PathBuf> = self
            .get_env_var(RoverEnvKey::ConfigHome)?
            .map(|p| Utf8PathBuf::from(&p));
        let override_api_key = self.get_env_var(RoverEnvKey::Key)?;
        Ok(Config::new(override_home.as_ref(), override_api_key)?)
    }

    pub(crate) fn get_client_config(&self) -> RoverResult<StudioClientConfig> {
        let override_endpoint = self.get_env_var(RoverEnvKey::RegistryUrl)?;
        let is_sudo = if let Some(fire_flower) = self.get_env_var(RoverEnvKey::FireFlower)? {
            let fire_flower = fire_flower.to_lowercase();
            fire_flower == "true" || fire_flower == "1"
        } else {
            false
        };
        let config = self.get_rover_config()?;
        Ok(StudioClientConfig::new(
            override_endpoint,
            config,
            is_sudo,
            self.get_reqwest_client_builder(),
            self.client_timeout,
        ))
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
                        .with_timeout(self.client_timeout.get_duration()),
                )
                .ok();
            self.get_reqwest_client_builder()
        }
    }

    pub(crate) fn get_checks_timeout_seconds(&self) -> RoverResult<u64> {
        if let Some(seconds) = self.get_env_var(RoverEnvKey::ChecksTimeoutSeconds)? {
            Ok(seconds.parse::<u64>()?)
        } else {
            // default to 5 minutes
            Ok(300)
        }
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
    /// Initialize a GraphQL API project using Apollo Federation with Apollo Router
    #[cfg(feature = "init")]
    Init(command::Init),

    /// Cloud configuration commands
    Cloud(command::Cloud),

    /// Configuration profile commands
    Config(command::Config),

    /// Contract configuration commands
    Contract(command::Contract),

    /// This command starts a local router that can query across one or more
    /// running GraphQL APIs (subgraphs) through one endpoint (supergraph).
    /// As you add, edit, and remove subgraphs, `rover dev` automatically
    /// composes all of their schemas into a new supergraph schema, and the
    /// router reloads.
    ///
    /// ⚠️ Do not run this command in production!
    /// ⚠️ It is intended for local development.
    ///
    /// You can navigate to the supergraph endpoint in your browser
    /// to execute operations and see query plans using Apollo Sandbox.
    Dev(command::Dev),

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
    #[command(hide = true)]
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
