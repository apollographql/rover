#[cfg(not(feature = "dev-next"))]
use std::io::{self, IsTerminal};

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::{self, Parser};
#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use clap::{error::ErrorKind as ClapErrorKind, CommandFactory};
#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use dialoguer::Input;
#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use rover_std::{Fs, Style};

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
use crate::cli::Rover;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// The name of the subgraph.
    #[arg(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct OptionalSubgraphOpts {
    /// The name of the subgraph.
    ///
    /// This must be unique to each `rover dev` process.
    #[arg(long = "name", short = 'n')]
    #[serde(skip_serializing)]
    subgraph_name: Option<String>,

    /// The URL that the `rover dev` router should use to communicate with a running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` process and cannot be the same endpoint used by the graph router, which are specified by the `--supergraph-port` and `--supergraph-address` arguments.
    #[arg(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    subgraph_url: Option<String>,

    /// The path to a GraphQL schema file that `rover dev` will use as this subgraph's schema.
    ///
    /// If this argument is passed, `rover dev` does not periodically introspect the running subgraph to obtain its schema.
    /// Instead, it watches the file at the provided path and recomposes the supergraph schema whenever changes occur.
    #[arg(long = "schema", short = 's', value_name = "SCHEMA_PATH")]
    #[serde(skip_serializing)]
    subgraph_schema_path: Option<Utf8PathBuf>,

    /// The number of seconds between introspection requests to the running subgraph.
    /// Only used when the `--schema` argument is not passed.
    /// The default value is 1 second.
    #[arg(
        long = "polling-interval",
        short = 'i',
        default_value = "1",
        conflicts_with = "subgraph_schema_path"
    )]
    #[serde(skip_serializing)]
    pub subgraph_polling_interval: u64,

    /// The number of times to retry a subgraph if an error is detected from it
    /// The default value is 0.
    #[arg(long = "subgraph-retries", short = 'r', default_value = "0")]
    #[serde(skip_serializing)]
    pub subgraph_retries: u64,
}

#[cfg(all(feature = "composition-js", not(feature = "dev-next")))]
impl OptionalSubgraphOpts {
    pub fn prompt_for_name(&self) -> Result<String> {
        if let Some(name) = &self.subgraph_name {
            Ok(name.to_string())
        } else if io::stderr().is_terminal() {
            let mut input = Input::new().with_prompt("what is the name of this subgraph?");
            if let Some(dirname) = Self::maybe_name_from_dir() {
                input = input.default(dirname);
            }
            let name: String = input.interact_text()?;
            Ok(name)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "--name <SUBGRAPH_NAME> is required when not attached to a TTY",
            )
            .exit();
        }
    }

    pub fn prompt_for_url(&self) -> Result<Url> {
        let url_context = |input| format!("'{}' is not a valid subgraph URL.", &input);
        if let Some(subgraph_url) = &self.subgraph_url {
            Ok(subgraph_url
                .parse()
                .with_context(|| url_context(subgraph_url))?)
        } else if io::stderr().is_terminal() {
            let input: String = Input::new()
                .with_prompt("what URL is your subgraph running on?")
                .interact_text()?;
            Ok(input.parse().with_context(|| url_context(&input))?)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "--url <SUBGRAPH_URL> is required when not attached to a TTY",
            )
            .exit();
        }
    }

    pub fn prompt_for_schema(&self) -> Result<Option<Utf8PathBuf>> {
        if let Some(schema) = &self.subgraph_schema_path {
            Fs::assert_path_exists(schema)?;
            Ok(Some(schema.clone()))
        } else {
            let possible_schemas: Vec<Utf8PathBuf> = Fs::get_dir_entries("./")
                .map(|entries| {
                    entries.flatten().filter_map(|entry| {
                        let mut result = None;
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_file() {
                                let entry_path = entry.path();
                                if let Some(extension) = entry_path.extension() {
                                    if extension == "graphql" || extension == "gql" {
                                        if let Some(file_stem) = entry_path.file_stem() {
                                            if !file_stem.contains("supergraph") {
                                                result = Some(entry.path().to_path_buf());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        result
                    })
                })?
                .collect();

            let warn_prefix = Style::WarningPrefix.paint("WARN:");
            match possible_schemas.len() {
                0 => {
                    eprintln!("{0} could not detect a schema in the current working directory. to watch a schema, pass the {1} argument", &warn_prefix, Style::Command.paint("'--schema <PATH>'"));
                    Ok(None)
                }
                1 => {
                    eprintln!("{0} if you would like to watch {1} for changes instead of introspecting every second, re-run this command with the {1} argument", &warn_prefix, Style::Command.paint(format!("'--schema {}'", possible_schemas[0])));
                    Ok(None)
                }
                _ => {
                    eprintln!("{0} detected multiple schemas in the current working directory. you can only watch one schema at a time. to watch a schema, pass the {1} argument", &warn_prefix, Style::Command.paint("'--schema <PATH>'"));
                    Ok(None)
                }
            }
        }
    }

    fn maybe_name_from_dir() -> Option<String> {
        std::env::current_dir()
            .ok()
            .and_then(|x| x.file_name().map(|x| x.to_string_lossy().to_lowercase()))
    }
}
