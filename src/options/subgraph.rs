use dialoguer::Input;
use reqwest::Url;
use saucer::{
    clap::{self, ErrorKind as ClapErrorKind},
    CommandFactory, Fs, Parser, Utf8PathBuf,
};
use serde::{Deserialize, Serialize};

use crate::{cli::Rover, utils::prompt_confirm_default_yes, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// The name of the subgraph
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct OptionalSubgraphOpts {
    /// The name of the subgraph.
    ///
    /// This must be unique to each `rover dev` session.
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    subgraph_name: Option<String>,

    /// The URL that the `rover dev` router should use to communicate with this running subgraph (e.g., http://localhost:4000).
    ///
    /// This must be unique to each `rover dev` session and cannot be the same endpoint used by the graph router, which are specified by the `--port` argument.
    #[clap(long = "url", short = 'u')]
    #[serde(skip_serializing)]
    subgraph_url: Option<Url>,

    /// The path to a GraphQL schema file that `rover dev` will use as this subgraph's schema.
    ///
    /// If this argument is passed, `rover dev` does not periodically introspect the running subgraph to obtain its schema.
    /// Instead, it watches the file at the provided path and recomposes the supergraph schema whenever changes occur.
    #[clap(long = "schema", short = 's')]
    #[serde(skip_serializing)]
    subgraph_schema_path: Option<Utf8PathBuf>,
}

impl OptionalSubgraphOpts {
    pub fn prompt_for_name(&self) -> Result<String> {
        if let Some(name) = &self.subgraph_name {
            Ok(name.to_string())
        } else if atty::is(atty::Stream::Stderr) {
            let mut input = Input::new();
            input.with_prompt("what is the name of this subgraph?");
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
        if let Some(subgraph_url) = &self.subgraph_url {
            Ok(subgraph_url.clone())
        } else if atty::is(atty::Stream::Stderr) {
            let input: String = Input::new()
                .with_prompt("what URL is your subgraph running on?")
                .interact_text()?;
            Ok(input.parse()?)
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
            Fs::assert_path_exists(schema, "")?;
            Ok(Some(schema.clone()))
        } else if atty::is(atty::Stream::Stderr) {
            let mut possible_schemas = Vec::new();
            if let Ok(entries) = Fs::get_dir_entries("./", "") {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "graphql" || extension == "gql" {
                                    possible_schemas.push(entry.path().to_path_buf());
                                }
                            }
                        }
                    }
                }
            }
            match possible_schemas.len() {
                0 => {
                    eprintln!("could not detect a schema in the current working directory. to watch a schema, pass the `--schema <PATH>` flag");
                    Ok(None)
                }
                1 => {
                    let path = possible_schemas[0].clone();

                    if atty::is(atty::Stream::Stderr) {
                        let answer = prompt_confirm_default_yes(&format!("would you like to watch {} for changes instead of introspecting every second?", &path))?;
                        if answer {
                            Ok(Some(path))
                        } else {
                            Ok(None)
                        }
                    } else {
                        eprintln!("if you would like to watch {} for changes instead of introspecting every second, pass the `--schema <PATH>` flag", &path);
                        Ok(None)
                    }
                }
                _ => {
                    eprintln!("detected multiple schemas in the current working directory. you can only watch one schema at a time. to watch a schema, pass the `--schema <PATH>` flag");
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    fn maybe_name_from_dir() -> Option<String> {
        std::env::current_dir()
            .ok()
            .and_then(|x| x.file_name().map(|x| x.to_string_lossy().to_lowercase()))
    }
}
