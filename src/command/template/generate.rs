use std::fmt;
use std::fmt::{Display, Formatter};

use anyhow::anyhow;
use base64::Engine;
use clap::Parser;
use console::Term;
use dialoguer::{Confirm, MultiSelect};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use houston::Profile;

use crate::command::template::templates::run_codegen;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Clone, Debug, Parser, Serialize)]
pub struct Generate {
    /// The URL to the OpenAPI document for the REST API
    #[arg(long = "openapi-url")]
    openapi_url: String,
    #[clap(flatten)]
    pub profile: ProfileOpt,
}

impl Generate {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let openapi_document_source = reqwest::blocking::get(&self.openapi_url)
            .map_err(|e| {
                anyhow!(
                    "Could not reach OpenAPI document at {}: {}",
                    self.openapi_url,
                    e
                )
            })?
            .text()
            .map_err(|e| {
                anyhow!(
                    "Could not download OpenAPI document at {}: {}",
                    self.openapi_url,
                    e
                )
            })?;
        let openapi_document: OpenAPI =
            serde_yaml::from_str(&openapi_document_source).map_err(|err| {
                anyhow!(
                    "Could not parse OpenAPI document at {}: {}",
                    self.openapi_url,
                    err
                )
            })?;
        let operations = openapi_document.into_operations();
        warn_users()?;
        let encoded_operations = select_operations(operations)?;
        eprintln!("Starting code generation, this will take some time.");
        let sandbox_url = run_codegen(
            self.openapi_url.clone(),
            encoded_operations,
            Profile::get_credential(&self.profile.profile_name, &client_config.config)?.api_key,
        )?;
        Ok(RoverOutput::GenerateSandboxUrl { sandbox_url })
    }
}

fn warn_users() -> RoverResult<()> {
    let confirmation = Confirm::new()
        .with_prompt(
            "This is an experimental feature which uses generative AI to produce code. \
        Carefully review the code after generation before running it. Continue?",
        )
        .interact_on(&Term::stderr())?;
    confirmation
        .then_some(())
        .ok_or_else(|| anyhow!("Code generation cancelled").into())
}

fn select_operations(operations: Vec<Operation>) -> RoverResult<Vec<String>> {
    let selected_operations = MultiSelect::new()
        .with_prompt("Select up to 10 operations to convert to GraphQL (use spacebar to select, enter to confirm)")
        .max_length(20)
        .items(&operations)
        .interact_on(&Term::stderr())?;
    if selected_operations.is_empty() {
        return Err(anyhow!("You must select at least one operation").into());
    }
    if selected_operations.len() > 10 {
        return Err(anyhow!("You can only select up to 10 operations").into());
    }
    Ok(selected_operations
        .into_iter()
        .filter_map(|index| operations.get(index))
        .map(|operation| operation.encode())
        .collect())
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAPI {
    paths: IndexMap<Path, IndexMap<Method, serde_yaml::Value>>,
}

impl OpenAPI {
    fn into_operations(self) -> Vec<Operation> {
        self.paths
            .into_iter()
            .flat_map(|(path, methods)| {
                methods
                    .into_iter()
                    .map(|(method, _)| Operation {
                        path: path.clone(),
                        method,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

struct Operation {
    path: Path,
    method: Method,
}

impl Operation {
    /// base64 encode the operation the way that the GraphQL API wants it
    fn encode(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", self.method, self.path))
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.method.0.to_ascii_uppercase(), self.path.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(transparent)]
struct Path(String);

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(transparent)]
struct Method(String);

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
