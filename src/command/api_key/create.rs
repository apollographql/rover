use std::collections::HashMap;
use std::io::IsTerminal;
use std::fs::canonicalize;
use std::path::PathBuf;

use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::operations::api_keys::create::{CreateKeyInput, run, SubgraphIdentifierInput, ApiKeyResourceInput};
use serde::Serialize;

use crate::command::api_keys::{ApiKeyType, OrganizationOpt};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverError, RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Create {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
    #[clap(name = "TYPE", value_enum, help = "The type of the API key")]
    key_type: ApiKeyType,
    #[clap(help = "The name of the key to be created")]
    name: String,
    #[clap(long)]
    subgraph_config: Option<PathBuf>,
}

impl Create {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resources = match self.key_type {
            ApiKeyType::Operator => None,
            ApiKeyType::Subgraph => {
                let file_descriptor = self.subgraph_config.clone()
                    .map(canonicalize)
                    .transpose()?
                    .map(Utf8PathBuf::from_path_buf)
                    .transpose()
                    .map_err(|p| anyhow::anyhow!("Unable to convert {:?} to Utf8PathBuf", p))?
                    .map(FileDescriptorType::File)
                    .unwrap_or_else(|| FileDescriptorType::Stdin);
                let mut stdin = std::io::stdin();
                if let FileDescriptorType::Stdin = file_descriptor {
                    if stdin.is_terminal() {
                        return Err(RoverError::new(
                            anyhow::anyhow!("Expected subgraph config from stdin, received none")
                        ).with_suggestion(
                            crate::RoverErrorSuggestion::Adhoc("Pipe supergraph config to stdin or provide a file path via the --subgraph-config flag".to_string()))
                        );
                    }
                }
                let content = file_descriptor.read_file_descriptor("subgraph config", &mut stdin)?;
                let config: SubgraphKeyConfig = serde_yaml::from_str(&content)?;
                let mut subgraphs_input = Vec::new();
                for (graph_id, variants) in config.iter() {
                    for (variant_name, subgraphs) in variants.iter() {
                        for subgraph_name in subgraphs {
                            subgraphs_input.push(SubgraphIdentifierInput {
                                graph_id: graph_id.clone(),
                                variant_name: variant_name.clone(),
                                subgraph_name: subgraph_name.to_string()
                            })
                        }
                    }
                }
                let resources = ApiKeyResourceInput {
                    subgraphs: Some(subgraphs_input),
                };
                Some(resources)
            }
        };
        let resp = run(
            CreateKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                name: self.name.clone(),
                key_type: self.key_type.into_query_enum(),
                resources
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::CreateKeyResponse {
            api_key: resp.token,
            key_type: self.key_type.to_string(),
            id: resp.key_id,
            name: resp.key_name,
        })
    }
}

pub type GraphId = String;
pub type VariantName = String;
pub type SubgraphName = String;
pub type SubgraphIdentifier = HashMap<VariantName, Vec<SubgraphName>>;
pub type SubgraphKeyConfig = HashMap<GraphId, SubgraphIdentifier>;
