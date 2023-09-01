use camino::Utf8PathBuf;
use clap::{self, Parser};
use serde::{Deserialize, Serialize};

#[cfg(feature = "composition-js")]
use anyhow::{Context, Result};

#[cfg(feature = "composition-js")]
use clap::{error::ErrorKind as ClapErrorKind, CommandFactory};

#[cfg(feature = "composition-js")]
use dialoguer::Input;

#[cfg(feature = "composition-js")]
use reqwest::Url;

#[cfg(feature = "composition-js")]
use rover_std::{Emoji, Fs, Style};

#[cfg(feature = "composition-js")]
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
}

#[cfg(feature = "composition-js")]
impl OptionalSubgraphOpts {
    pub fn prompt_for_name(&self) -> Result<String> {
        if let Some(name) = &self.subgraph_name {
            Ok(name.to_string())
        } else if atty::is(atty::Stream::Stderr) {
            let mut input = Input::new();
            input.with_prompt(format!(
                "{}what is the name of this subgraph?",
                Emoji::Person
            ));
            if let Some(dirname) = Self::maybe_name_from_dir() {
                input.default(dirname);
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
        } else if atty::is(atty::Stream::Stderr) {
            let input: String = Input::new()
                .with_prompt(format!(
                    "{}what URL is your subgraph running on?",
                    Emoji::Web
                ))
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

    pub fn get_subgraph_watcher(
        &self,
        router_socket_addr: SocketAddr,
        client_config: &StudioClientConfig,
        follower_messenger: FollowerMessenger,
    ) -> RoverResult<SubgraphSchemaSource> {
        tracing::info!("checking version");
        follower_messenger.version_check()?;
        tracing::info!("checking for existing subgraphs");
        let session_subgraphs = follower_messenger.session_subgraphs()?;
        let url = self.prompt_for_url()?;
        let normalized_user_urls = normalize_loopback_urls(&url);
        let normalized_supergraph_urls = normalize_loopback_urls(
            &Url::parse(&format!("http://{}", router_socket_addr)).unwrap(),
        );

        for normalized_user_url in &normalized_user_urls {
            for normalized_supergraph_url in &normalized_supergraph_urls {
                if normalized_supergraph_url == normalized_user_url {
                    let mut err = RoverError::new(anyhow!("The subgraph argument `--url {}` conflicts with the supergraph argument `--supergraph-port {}`", &url, normalized_supergraph_url.port().unwrap()));
                    if session_subgraphs.is_none() {
                        err.set_suggestion(RoverErrorSuggestion::Adhoc("Set the `--supergraph-port` flag to a different port to start the local supergraph.".to_string()))
                    } else {
                        err.set_suggestion(RoverErrorSuggestion::Adhoc("Start your subgraph on a different port and re-run this command with the new `--url`.".to_string()))
                    }
                    return Err(err);
                }
            }
        }

        let name = self.prompt_for_name()?;
        let schema = self.prompt_for_schema()?;

        if let Some(session_subgraphs) = session_subgraphs {
            for (session_subgraph_name, session_subgraph_url) in session_subgraphs {
                if session_subgraph_name == name {
                    return Err(RoverError::new(anyhow!(
                        "subgraph with name '{}' is already running in this `rover dev` session",
                        &name
                    )));
                }
                let normalized_session_urls = normalize_loopback_urls(&session_subgraph_url);
                for normalized_user_url in &normalized_user_urls {
                    for normalized_session_url in &normalized_session_urls {
                        if normalized_session_url == normalized_user_url {
                            return Err(RoverError::new(anyhow!(
                                "subgraph with url '{}' is already running in this `rover dev` session",
                                &url
                            )));
                        }
                    }
                }
            }
        }

        if let Some(schema) = schema {
            SubgraphSchemaSource::new_from_file_path((name, url), schema)
        } else {
            let client = client_config
                .get_builder()
                .with_timeout(Duration::from_secs(5))
                .build()?;
            SubgraphSchemaSource::new_from_url(
                (name, url),
                client,
                self.subgraph_polling_interval,
                None,
            )
        }
    }
}
