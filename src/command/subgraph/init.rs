use console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use dialoguer::Select;
use rover_client::shared::GraphRef;
use saucer::{anyhow, clap, Parser, Utf8PathBuf};
use serde::Serialize;

use crate::command::RoverOutput;
use crate::dot_apollo::{DotApollo, MultiSubgraphConfig, SubgraphConfig};
use crate::error::RoverError;
use crate::utils::parsers::FileDescriptorType;
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    #[clap(long)]
    supergraph_id: Option<String>,

    #[clap(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    #[clap(long)]
    #[serde(skip_serializing)]
    local_endpoint: Option<String>,

    #[clap(long)]
    #[serde(skip_serializing)]
    remote_endpoint: Option<String>,

    #[clap(long)]
    #[serde(skip_serializing)]
    subgraph_name: Option<String>,

    #[clap(long, conflicts_with_all(&["schema-url", "schema-ref", "schema-ref-subgraph-name"]))]
    #[serde(skip_serializing)]
    schema_file: Option<FileDescriptorType>,

    #[clap(long, conflicts_with_all(&["schema-file", "schema-ref", "schema-ref-subgraph-name"]))]
    #[serde(skip_serializing)]
    schema_url: Option<String>,

    #[clap(long, requires("schema-ref-subgraph-name"), conflicts_with_all(&["schema-file", "schema-url"]))]
    schema_ref: Option<GraphRef>,

    #[clap(long, requires("schema-ref"), conflicts_with_all(&["schema-file", "schema-url"]))]
    schema_ref_subgraph_name: Option<String>,
}

impl Init {
    pub fn run(&self) -> Result<RoverOutput> {
        let mut new_subgraph_project = MultiSubgraphConfig::new();
        let name = self.get_subgraph_name()?;
        let local_endpoint = self.get_local_endpoint()?;
        let remote_endpoint = self.get_remote_endpoint()?;
        let subgraph_config = match (
            &self.schema_file,
            &self.schema_url,
            &self.schema_ref,
            &self.schema_ref_subgraph_name,
        ) {
            (Some(schema_file), None, None, None) => match schema_file {
                FileDescriptorType::File(file) => {
                    SubgraphConfig::from_file(file, local_endpoint, remote_endpoint)
                }
                FileDescriptorType::Stdin => {
                    return Err(RoverError::new(anyhow!(
                        "stdin is not a valid schema source for .apollo configuration"
                    )))
                }
            },
            (None, Some(subgraph_url), None, None) => SubgraphConfig::from_subgraph_introspect(
                subgraph_url.clone(),
                local_endpoint,
                remote_endpoint,
            ),
            (None, None, Some(schema_graphref), Some(schema_ref_subgraph_name)) => {
                SubgraphConfig::from_studio(
                    schema_graphref.to_string(),
                    Some(schema_ref_subgraph_name.to_string()),
                    local_endpoint,
                    remote_endpoint,
                )
            }
            _ => {
                let local_file = "local file";
                let introspect = "introspection url";
                let studio_subgraph = "apollo studio subgraph";
                let source_opts = vec![local_file, introspect, studio_subgraph];
                let source_opt = Select::with_theme(&ColorfulTheme::default())
                    .items(&source_opts)
                    .default(0)
                    .interact_on_opt(&Term::stderr())?;
                match source_opt {
                    Some(i) => {
                        let selected = source_opts[i];
                        eprintln!("✅  selected {}...", selected);
                        match selected {
                            "local file" => {
                                let file: Utf8PathBuf = Input::new()
                                    .with_prompt("What is the path to your schema?")
                                    .interact_text()?;
                                SubgraphConfig::from_file(file, local_endpoint, remote_endpoint)
                            }
                            "introspection url" => {
                                let subgraph_url: String = Input::new()
                                    .with_prompt("What is the endpoint to introspect?")
                                    .interact_text()?;
                                SubgraphConfig::from_subgraph_introspect(
                                    subgraph_url,
                                    local_endpoint,
                                    remote_endpoint,
                                )
                            }
                            "apollo studio subgraph" => {
                                let graphref: String = Input::new()
                                    .with_prompt("What is the Apollo Studio graphref?")
                                    .interact_text()?;
                                let subgraph: String = Input::new()
                                    .with_prompt("What is the name of the subgraph?")
                                    .interact_text()?;
                                SubgraphConfig::from_studio(
                                    graphref,
                                    Some(subgraph),
                                    local_endpoint,
                                    remote_endpoint,
                                )
                            }
                            _ => unreachable!(),
                        }
                    }
                    None => {
                        unreachable!()
                    }
                }
            }
        };
        new_subgraph_project
            .subgraph()
            .name(name)
            .config(subgraph_config)
            .add()?;
        let dot_apollo = DotApollo::new_subgraph(new_subgraph_project)?;
        dot_apollo.write_yaml_to_fs()?;
        Ok(RoverOutput::EmptySuccess)
    }

    fn get_subgraph_name(&self) -> Result<String> {
        if let Some(name) = &self.subgraph_name {
            Ok(name.to_string())
        } else {
            let name = Input::new()
                .with_prompt("What is the name of your subgraph?")
                .interact_text()?;
            Ok(name)
        }
    }

    fn get_local_endpoint(&self) -> Result<String> {
        if let Some(local_endpoint) = &self.local_endpoint {
            Ok(local_endpoint.to_string())
        } else {
            let local_endpoint: String = Input::new()
                .with_prompt("What URL does your subgraph run on locally?")
                .default("http://localhost:4000/".to_string())
                .interact_text()?;

            Ok(local_endpoint)
        }
    }

    fn get_remote_endpoint(&self) -> Result<Option<String>> {
        if let Some(remote_endpoint) = &self.remote_endpoint {
            Ok(Some(remote_endpoint.to_string()))
        } else {
            let local_endpoint: String = Input::new()
                .with_prompt("What URL does your subgraph run on when it is deployed? (optional)")
                .default("".to_string())
                .interact_text()?;
            if local_endpoint.is_empty() {
                Ok(None)
            } else {
                Ok(Some(local_endpoint.to_string()))
            }
        }
    }
}
