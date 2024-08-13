use crate::blocking::StudioClient;
use crate::operations::graph::variant::VariantListInput;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/variant/variant_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. variant_list_query
pub(crate) struct VariantListQuery;

/// The main function to be used from this module.
/// This function lists all the variants for a given graph ref
pub async fn run(input: VariantListInput, client: &StudioClient) -> Result<(), RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<VariantListQuery>(input.into()).await?;
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let mut valid_variants = Vec::new();

    for variant in graph.variants {
        valid_variants.push(variant.name)
    }

    if !valid_variants.contains(&graph_ref.variant) {
        Err(RoverClientError::NoSchemaForVariant {
            graph_ref,
            valid_variants,
            frontend_url_root: response_data.frontend_url_root,
        })
    } else {
        Ok(())
    }
}
