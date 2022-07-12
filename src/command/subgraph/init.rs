use ansi_term::Colour::Cyan;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use clap::Parser;
use reqwest::Url;
use rover_client::shared::GraphRef;
use serde::Serialize;

use rover_client::operations::subgraph::list::{self, SubgraphListInput};

use crate::command::RoverOutput;
use crate::dot_apollo::{DotApollo, SubgraphProjectConfig};
use crate::options::{OptionalGraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::Result;

use std::io;

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

    #[clap(long, conflicts_with_all(&["schema-url", "schema-ref", "schema-ref-subgraph-name"]))]
    #[serde(skip_serializing)]
    schema_file: Option<FileDescriptorType>,

    #[clap(long, conflicts_with_all(&["schema-file", "schema-ref", "schema-ref-subgraph-name"]))]
    #[serde(skip_serializing)]
    schema_url: Option<Url>,

    #[clap(long, requires("schema-ref-subgraph-name"), conflicts_with_all(&["schema-file", "schema-url"]))]
    schema_ref: Option<GraphRef>,

    #[clap(long, requires("schema-ref"), conflicts_with_all(&["schema-file", "schema-url"]))]
    schema_ref_subgraph_name: Option<String>,
}

impl Init {
    pub fn run(&self) -> Result<RoverOutput> {
        let schema_source = match (
            &self.schema_file,
            &self.schema_url,
            &self.schema_ref,
            &self.schema_ref_subgraph_name,
        ) {
            (Some(schema_file), None, None, None) => match schema_file {
                FileDescriptorType::File(file) => Some(SchemaSource::File { file: file.clone() }),
                FileDescriptorType::Stdin => {
                    let sdl =
                        schema_file.read_file_descriptor("--schema-file", &mut io::stdin())?;
                    Some(SchemaSource::Sdl { sdl })
                }
            },
            (None, Some(subgraph_url), None, None) => Some(SchemaSource::SubgraphIntrospection {
                subgraph_url: subgraph_url.clone(),
            }),
            (None, None, Some(schema_graphref), Some(schema_ref_subgraph_name)) => {
                Some(SchemaSource::Subgraph {
                    graphref: schema_graphref.to_string(),
                    subgraph: schema_ref_subgraph_name.clone(),
                })
            }
            _ => None,
        };
        let subgraph_config = SubgraphConfig {
            routing_url: self.remote_endpoint.clone(),
            schema: schema_source.unwrap(),
        };
        let project_config =
            SubgraphProjectConfig::new(self.supergraph_id.clone(), subgraph_config);
        let dot_apollo = DotApollo::new_subgraph(project_config)?;
        dot_apollo.write_yaml_to_fs()?;
        Ok(RoverOutput::EmptySuccess)
    }
}
