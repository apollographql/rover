use camino::Utf8PathBuf;
use reqwest::blocking::Client;
use serde::Serialize;
use structopt::{clap::AppSettings, StructOpt};

use crate::command::output::JsonOutput;
use crate::command::{self, RoverOutput};
use crate::utils::{
    client::StudioClientConfig,
    env::{RoverEnv, RoverEnvKey},
    stringify::option_from_display,
    version,
};
use crate::Result;

use config::Config;
use houston as config;
use rover_client::shared::GitContext;
use sputnik::Session;
use timber::{Level, LEVELS};

use std::{process, thread};

#[derive(Debug, Serialize, StructOpt)]
#[structopt(
    name = "Rover", 
    global_settings = &[
        AppSettings::ColoredHelp,
        AppSettings::StrictUtf8,
        AppSettings::VersionlessSubcommands,
    ],
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
")]
pub struct Rover {
    #[structopt(subcommand)]
    command: Command,

    /// Specify Rover's log level
    #[structopt(long = "log", short = "l", global = true, possible_values = &LEVELS, case_insensitive = true)]
    #[serde(serialize_with = "option_from_display")]
    log_level: Option<Level>,

    /// Use json output
    #[structopt(long = "json", global = true)]
    json: bool,

    #[structopt(skip)]
    #[serde(skip_serializing)]
    pub(crate) env_store: RoverEnv,

    #[structopt(skip)]
    #[serde(skip_serializing)]
    client: Client,
}

impl Rover {
    pub fn run(&self) -> ! {
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
                if self.json {
                    println!("{}", JsonOutput::success(output));
                } else {
                    output.print();
                }
                process::exit(0);
            }
            Err(error) => {
                if self.json {
                    println!("{}", JsonOutput::error(error));
                } else {
                    tracing::debug!(?error);
                    eprint!("{}", error);
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
            Command::Supergraph(command) => command.run(self.get_client_config()?),
            Command::Docs(command) => command.run(),
            Command::Graph(command) => {
                command.run(self.get_client_config()?, self.get_git_context()?)
            }
            Command::Subgraph(command) => {
                command.run(self.get_client_config()?, self.get_git_context()?)
            }
            Command::Update(command) => {
                command.run(self.get_rover_config()?, self.get_reqwest_client())
            }
            Command::Install(command) => command.run(self.get_install_override_path()?),
            Command::Info(command) => command.run(),
            Command::Explain(command) => command.run(),
        }
    }

    pub(crate) fn get_rover_config(&self) -> Result<Config> {
        let override_home: Option<Utf8PathBuf> = self
            .env_store
            .get(RoverEnvKey::ConfigHome)?
            .map(|p| Utf8PathBuf::from(&p));
        let override_api_key = self.env_store.get(RoverEnvKey::Key)?;
        Ok(Config::new(override_home.as_ref(), override_api_key)?)
    }

    pub(crate) fn get_client_config(&self) -> Result<StudioClientConfig> {
        let override_endpoint = self.env_store.get(RoverEnvKey::RegistryUrl)?;
        let config = self.get_rover_config()?;
        Ok(StudioClientConfig::new(
            override_endpoint,
            config,
            self.get_reqwest_client(),
        ))
    }

    pub(crate) fn get_install_override_path(&self) -> Result<Option<Utf8PathBuf>> {
        Ok(self
            .env_store
            .get(RoverEnvKey::Home)?
            .map(|p| Utf8PathBuf::from(&p)))
    }

    pub(crate) fn get_git_context(&self) -> Result<GitContext> {
        // constructing GitContext with a set of overrides from env vars
        let override_git_context = GitContext {
            branch: self.env_store.get(RoverEnvKey::VcsBranch).ok().flatten(),
            commit: self.env_store.get(RoverEnvKey::VcsCommit).ok().flatten(),
            author: self.env_store.get(RoverEnvKey::VcsAuthor).ok().flatten(),
            remote_url: self.env_store.get(RoverEnvKey::VcsRemoteUrl).ok().flatten(),
        };

        let git_context = GitContext::new_with_override(override_git_context);
        tracing::debug!(?git_context);
        Ok(git_context)
    }

    pub(crate) fn get_reqwest_client(&self) -> Client {
        // we can use clone here freely since `reqwest` uses an `Arc` under the hood
        self.client.clone()
    }
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Configuration profile commands
    Config(command::Config),

    /// Supergraph schema commands
    Supergraph(command::Supergraph),

    /// Graph API schema commands
    Graph(command::Graph),

    /// Subgraph schema commands
    Subgraph(command::Subgraph),

    /// Interact with Rover's documentation
    Docs(command::Docs),

    /// Commands related to updating rover
    Update(command::Update),

    /// Installs Rover
    #[structopt(setting(structopt::clap::AppSettings::Hidden))]
    Install(command::Install),

    /// Get system information
    #[structopt(setting(structopt::clap::AppSettings::Hidden))]
    Info(command::Info),

    /// Explain error codes
    Explain(command::Explain),
}
