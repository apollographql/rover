use calm_io::stdoutln;
use lazycell::{AtomicLazyCell, LazyCell};
use reqwest::blocking::Client;
use saucer::Utf8PathBuf;
use saucer::{clap, AppSettings, Parser};
use serde::Serialize;

use crate::command::output::JsonOutput;
use crate::command::{self, RoverOutput};
use crate::utils::{
    client::{ClientBuilder, ClientTimeout, StudioClientConfig},
    env::{RoverEnv, RoverEnvKey},
    stringify::option_from_display,
    version,
};
use crate::{anyhow, Result};

use config::Config;
use houston as config;
use rover_client::shared::GitContext;
use sputnik::Session;
use timber::{Level, LEVELS};

use std::{io, process, str::FromStr, thread};

#[derive(Debug, Serialize, Parser)]
#[clap(
    name = "Rover",
    author,
    version,
    about = "
Rover - Your Graph Companion
Read the getting started guide by running:

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
pub struct Rover {
    #[clap(subcommand)]
    command: Command,

    /// Specify Rover's log level
    #[clap(long = "log", short = 'l', global = true, possible_values = &LEVELS, case_insensitive = true)]
    #[serde(serialize_with = "option_from_display")]
    log_level: Option<Level>,

    /// Specify Rover's output type
    #[clap(long = "output", default_value = "plain", possible_values = &["json", "plain"], case_insensitive = true, global = true)]
    output_type: OutputType,

    /// Accept invalid certificates when performing HTTPS requests.
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If invalid certificates are trusted, any certificate for any site will be trusted for use.
    /// This includes expired certificates.
    /// This introduces significant vulnerabilities, and should only be used as a last resort.
    #[clap(long = "insecure-accept-invalid-certs", global = true)]
    accept_invalid_certs: bool,

    /// Accept invalid hostnames when performing HTTPS requests.
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If hostname verification is not used, any valid certificate for any site will be trusted for use from any other.
    /// This introduces a significant vulnerability to man-in-the-middle attacks.
    #[clap(long = "insecure-accept-invalid-hostnames", global = true)]
    accept_invalid_hostnames: bool,

    /// Configure the timeout length (in seconds) when performing HTTP(S) requests.
    #[clap(
        long = "client-timeout",
        case_insensitive = true,
        global = true,
        default_value_t = ClientTimeout::default()
    )]
    client_timeout: ClientTimeout,

    #[clap(skip)]
    #[serde(skip_serializing)]
    env_store: LazyCell<RoverEnv>,

    #[clap(skip)]
    #[serde(skip_serializing)]
    client: AtomicLazyCell<Client>,
}

impl Rover {
    pub fn run_from_args() -> io::Result<()> {
        Rover::from_args().run()
    }

    pub fn run(&self) -> io::Result<()> {
        timber::init(self.log_level);
        tracing::trace!(command_structure = ?self);

        // attempt to create a new `Session` to capture anonymous usage data
        let rover_output = match Session::new(self) {
            // if successful, report the usage data in the background
            Ok(session) => {
                // kicks off the reporting on a background thread
                let report_thread = thread::spawn(move || {
                    // log + ignore errors because it is not in the critical path
                    let _ = session.report().map_err(|telemetry_error| {
                        tracing::debug!(?telemetry_error);
                        telemetry_error
                    });
                });

                // kicks off the app on the main thread
                // don't return an error with ? quite yet
                // since we still want to report the usage data
                let app_result = self.execute_command();

                // makes sure the reporting finishes in the background
                // before continuing.
                // ignore errors because it is not in the critical path
                let _ = report_thread.join();

                // return result of app execution
                // now that we have reported our usage data
                app_result
            }

            // otherwise just run the app without reporting
            Err(_) => self.execute_command(),
        };

        match rover_output {
            Ok(output) => {
                match self.output_type {
                    OutputType::Plain => output.print()?,
                    OutputType::Json => stdoutln!("{}", JsonOutput::from(output))?,
                }
                process::exit(0);
            }
            Err(error) => {
                match self.output_type {
                    OutputType::Json => stdoutln!("{}", JsonOutput::from(error))?,
                    OutputType::Plain => {
                        tracing::debug!(?error);
                        error.print()?;
                    }
                }
                process::exit(1);
            }
        }
    }

    pub fn execute_command(&self) -> Result<RoverOutput> {
        // before running any commands, we check if rover is up to date
        // this only happens once a day automatically
        // we skip this check for the `rover update` commands, since they
        // do their own checks

        if let Command::Update(_) = &self.command { /* skip check */
        } else {
            let config = self.get_rover_config();
            if let Ok(config) = config {
                let _ = version::check_for_update(config, false, self.get_reqwest_client());
            }
        }

        match &self.command {
            Command::Config(command) => command.run(self.get_client_config()?),
            Command::Fed2(command) => command.run(self.get_client_config()?),
            Command::Supergraph(command) => {
                command.run(self.get_install_override_path()?, self.get_client_config()?)
            }
            Command::Docs(command) => command.run(),
            Command::Graph(command) => command.run(
                self.get_client_config()?,
                self.get_git_context()?,
                self.get_checks_timeout_seconds()?,
            ),
            Command::Readme(command) => command.run(self.get_client_config()?),
            Command::Subgraph(command) => command.run(
                self.get_client_config()?,
                self.get_git_context()?,
                self.get_checks_timeout_seconds()?,
            ),
            Command::Update(command) => {
                command.run(self.get_rover_config()?, self.get_reqwest_client())
            }
            Command::Install(command) => {
                command.run(self.get_install_override_path()?, self.get_client_config()?)
            }
            Command::Info(command) => command.run(),
            Command::Explain(command) => command.run(),
        }
    }

    pub(crate) fn get_rover_config(&self) -> Result<Config> {
        let override_home: Option<Utf8PathBuf> = self
            .get_env_var(RoverEnvKey::ConfigHome)?
            .map(|p| Utf8PathBuf::from(&p));
        let override_api_key = self.get_env_var(RoverEnvKey::Key)?;
        Ok(Config::new(override_home.as_ref(), override_api_key)?)
    }

    pub(crate) fn get_client_config(&self) -> Result<StudioClientConfig> {
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
            self.get_reqwest_client(),
        ))
    }

    pub(crate) fn get_install_override_path(&self) -> Result<Option<Utf8PathBuf>> {
        Ok(self
            .get_env_var(RoverEnvKey::Home)?
            .map(|p| Utf8PathBuf::from(&p)))
    }

    pub(crate) fn get_git_context(&self) -> Result<GitContext> {
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

    pub(crate) fn get_reqwest_client(&self) -> Client {
        // return a clone of the underlying client if it's already been populated
        if let Some(client) = self.client.borrow() {
            // we can use clone here freely since `reqwest` uses an `Arc` under the hood
            client.clone()
        } else {
            // if a request hasn't been made yet, this cell won't be populated yet
            self.client
                .fill(
                    ClientBuilder::new()
                        .accept_invalid_certs(self.accept_invalid_certs)
                        .accept_invalid_hostnames(self.accept_invalid_hostnames)
                        .with_timeout(self.client_timeout.get_duration())
                        .build()
                        .expect("Could not configure the request client"),
                )
                .expect("Could not overwrite the existing request client");
            self.get_reqwest_client()
        }
    }

    pub(crate) fn get_checks_timeout_seconds(&self) -> Result<u64> {
        // default to 5 minutes
        Ok(self
            .get_env_var(RoverEnvKey::ChecksTimeoutSeconds)?
            .unwrap_or_else(|| "300".to_string())
            .parse::<u64>()
            .unwrap())
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
    /// Configuration profile commands
    Config(command::Config),

    /// (deprecated) Federation 2 Alpha commands
    #[clap(setting(AppSettings::Hidden))]
    Fed2(command::Fed2),

    /// Supergraph schema commands
    Supergraph(command::Supergraph),

    /// Graph API schema commands
    Graph(command::Graph),

    /// Readme commands
    Readme(command::Readme),

    /// Subgraph schema commands
    Subgraph(command::Subgraph),

    /// Interact with Rover's documentation
    Docs(command::Docs),

    /// Commands related to updating rover
    Update(command::Update),

    /// Installs Rover
    #[clap(setting(AppSettings::Hidden))]
    Install(command::Install),

    /// Get system information
    #[clap(setting(AppSettings::Hidden))]
    Info(command::Info),

    /// Explain error codes
    Explain(command::Explain),
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum OutputType {
    Plain,
    Json,
}

impl FromStr for OutputType {
    type Err = saucer::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input {
            "plain" => Ok(Self::Plain),
            "json" => Ok(Self::Json),
            _ => Err(anyhow!("Invalid output type.")),
        }
    }
}

impl Default for OutputType {
    fn default() -> Self {
        OutputType::Plain
    }
}
