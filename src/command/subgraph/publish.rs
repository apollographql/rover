use ansi_term::Colour::{Cyan, Yellow};
use dialoguer::Input;
use saucer::{anyhow, clap, Parser};
use serde::Serialize;

use crate::command::RoverOutput;
use crate::dot_apollo::DotApollo;
use crate::options::{OptionalGraphRefOpt, OptionalSchemaOpt, OptionalSubgraphOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::Result;

use rover_client::operations::subgraph::publish::{self, SubgraphPublishInput};
use rover_client::shared::{GitContext, GraphRef};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    #[clap(flatten)]
    subgraph: OptionalSubgraphOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: OptionalSchemaOpt,

    /// Indicate whether to convert a non-federated graph into a subgraph
    #[clap(short, long)]
    convert: bool,

    /// Url of a running subgraph that a gateway can route operations to
    /// (often a deployed subgraph). May be left empty ("") or a placeholder url
    /// if not running a gateway in managed federation mode
    #[clap(long)]
    #[serde(skip_serializing)]
    routing_url: Option<String>,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverOutput> {
        let mut maybe_multi_config = DotApollo::subgraph_from_yaml()?;
        let mut new_multi_config = maybe_multi_config.clone();
        let mut config_edited = false;
        let (subgraph_name, maybe_subgraph_config, maybe_supergraph_id) =
            if let Some(name) = &self.subgraph.subgraph_name {
                Ok(if let Some(multi_config) = maybe_multi_config {
                    (
                        name.to_string(),
                        multi_config.get_subgraph(&name),
                        multi_config.get_supergraph().graph_id(),
                    )
                } else {
                    (name.to_string(), None, None)
                })
            } else {
                if let Some(multi_config) = maybe_multi_config.as_mut() {
                    let (name, config) = multi_config.try_get_only_subgraph()?;
                    Ok((
                        name.to_string(),
                        Some(config),
                        multi_config.get_supergraph().graph_id(),
                    ))
                } else {
                    Err(anyhow!("you must specify a subgraph name to publish to"))
                }
            }?;

        let graph_id = if let Some(graph_id) = self.graph.graph_id() {
            graph_id
        } else {
            if let Some(supergraph_id) = maybe_supergraph_id {
                supergraph_id
            } else {
                let graph_id: String = Input::new()
                    .with_prompt("What is the name of the supergraph you want to extend?")
                    .interact_text()?;
                if let Some(new_multi_config) = new_multi_config.as_mut() {
                    config_edited = true;
                    new_multi_config
                        .supergraph()
                        .graph_id(graph_id.to_string())
                        .extend()?;
                }
                graph_id
            }
        };

        let routing_url = if let Some(routing_url) = &self.routing_url {
            Some(routing_url.to_string())
        } else {
            if let Some(subgraph_config) = &maybe_subgraph_config {
                if let Some(routing_url) = &subgraph_config.remote_endpoint {
                    Some(routing_url.to_string())
                } else {
                    // TODO: check to see if we've deployed to that graph ref
                    // before and if it has a subgraph routing url
                    let routing_url: String = Input::new()
                        .with_prompt("What endpoint is your subgraph deployed to?")
                        .interact_text()?;
                    if let Some(new_multi_config) = new_multi_config.as_mut() {
                        config_edited = true;
                        new_multi_config.edit_subgraph(&subgraph_name, &routing_url)?;
                    }
                    Some(routing_url)
                }
            } else {
                None
            }
        };

        let variant = self.graph.variant();

        let graph_ref = GraphRef {
            name: graph_id,
            variant: variant.unwrap_or("current".to_string()),
        };

        let schema = if let Some(schema) = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?
        {
            Ok(schema)
        } else {
            if let Some(subgraph_config) = &maybe_subgraph_config {
                Ok(subgraph_config
                    .schema
                    .resolve(&client_config, &self.profile.profile_name)?)
            } else {
                Err(anyhow!("you must specify a schema to publish"))
            }
        }?;

        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref.to_string()),
            Cyan.normal().paint(&subgraph_name),
            Yellow.normal().paint(&self.profile.profile_name)
        );

        tracing::debug!("Publishing \n{}", &schema);

        let publish_response = publish::run(
            SubgraphPublishInput {
                graph_ref: graph_ref.clone(),
                subgraph: subgraph_name.clone(),
                url: routing_url.clone(),
                schema,
                git_context,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )?;

        if let Some(new_multi_config) = new_multi_config {
            if config_edited {
                DotApollo::new_subgraph(new_multi_config)?.write_yaml_to_fs()?
            }
        }

        Ok(RoverOutput::SubgraphPublishResponse {
            graph_ref: graph_ref.clone(),
            subgraph: subgraph_name.clone(),
            publish_response,
        })
    }
}
