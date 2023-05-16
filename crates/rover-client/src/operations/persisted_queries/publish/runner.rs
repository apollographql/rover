use crate::blocking::StudioClient;
use crate::operations::persisted_queries::publish::{
    PersistedQueriesPublishInput, PersistedQueriesPublishResponse,
};
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct QueriesPersistMutation;

pub fn run(
    input: PersistedQueriesPublishInput,
    client: &StudioClient,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<QueriesPersistMutation>(input.into())?;
    build_response(data, graph_ref)
}

fn build_response(
    data: queries_persist_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let valid_variants = graph.variants.iter().map(|it| it.name.clone()).collect();

    let variant = graph.variant.ok_or(RoverClientError::NoSchemaForVariant {
        graph_ref: graph_ref.clone(),
        valid_variants,
        frontend_url_root: data.frontend_url_root,
    })?;

    Ok(PersistedQueriesPublishResponse { graph_ref })
}

#[cfg(test)]
mod tests {
    use crate::shared::GraphRef;

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[test]
    fn get_readme_from_response_data_works() {
        unimplemented!()
    }
}
