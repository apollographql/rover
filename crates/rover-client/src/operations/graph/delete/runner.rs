use crate::blocking::StudioClient;
use crate::operations::graph::{
    delete::GraphDeleteInput,
    variant::{self, VariantListInput},
};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/delete/delete_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_delete_mutation
pub(crate) struct GraphDeleteMutation;

/// The main function to be used from this module.
/// This function deletes a single graph variant from the graph registry
pub async fn run(input: GraphDeleteInput, client: &StudioClient) -> Result<(), RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = match client.post::<GraphDeleteMutation>(input.into()).await {
        Ok(data) => data,
        Err(e) => {
            if e.to_string().contains("Variant not found") {
                variant::run(
                    VariantListInput {
                        graph_ref: graph_ref.clone(),
                    },
                    client,
                )
                .await?;
            }
            return Err(e);
        }
    };

    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let variant = graph
        .variant
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    if variant.delete.deleted {
        Ok(())
    } else {
        Err(RoverClientError::AdhocError {
            msg: "An unknown error occurred while deleting your graph.".to_string(),
        })
    }
}
