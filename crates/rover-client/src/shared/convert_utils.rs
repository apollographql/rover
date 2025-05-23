use crate::blocking::StudioClient;
use crate::operations::config::is_federated;
use crate::operations::config::is_federated::IsFederatedInput;
use crate::operations::graph::variant;
use crate::operations::graph::variant::VariantListInput;
use crate::shared::GraphRef;
use crate::RoverClientError;

pub async fn should_convert_to_federated_graph(
    graph_ref: &GraphRef,
    convert_to_federated_graph: bool,
    client: &StudioClient,
) -> Result<(), RoverClientError> {
    // We don't want to implicitly convert non-federated graph to supergraphs.
    // Error here if no --convert flag is passed _and_ the current context
    // is non-federated. Add a suggestion to require a --convert flag.
    if !convert_to_federated_graph {
        // first, check if the variant exists _at all_
        // if it doesn't exist, there is no graph schema to "convert"
        // so don't require --convert in this case, just publish the subgraph
        let variant_exists = variant::run(
            VariantListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await
        .is_ok();

        if variant_exists {
            // check if subgraphs have ever been published to this graph ref
            let is_federated = is_federated::run(
                IsFederatedInput {
                    graph_ref: graph_ref.clone(),
                },
                client,
            )
            .await?;

            if !is_federated {
                return Err(RoverClientError::ExpectedFederatedGraph {
                    graph_ref: graph_ref.clone(),
                    can_operation_convert: true,
                });
            }
        } else {
            tracing::debug!("Publishing new subgraph(s) to {}", &graph_ref);
        }
    }
    Ok(())
}
