use super::types::{PersistedQueryList, ResolvePersistedQueryListInput};
use crate::blocking::StudioClient;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/resolve/resolve_pql_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ResolvePersistedQueryListQuery;

pub async fn run(
    input: ResolvePersistedQueryListInput,
    client: &StudioClient,
) -> Result<PersistedQueryList, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client
        .post::<ResolvePersistedQueryListQuery>(input.into())
        .await?;
    build_response(data, graph_ref)
}

fn build_response(
    data: resolve_persisted_query_list_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<PersistedQueryList, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let valid_variants = graph
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect();

    let variant = graph.variant.ok_or(RoverClientError::NoSchemaForVariant {
        graph_ref: graph_ref.clone(),
        valid_variants,
        frontend_url_root: data.frontend_url_root.clone(),
    })?;

    if let Some(persisted_query_list) = variant.persisted_query_list {
        Ok(PersistedQueryList {
            graph_ref,
            id: persisted_query_list.id,
            name: persisted_query_list.name,
        })
    } else {
        Err(RoverClientError::NoPersistedQueryList {
            graph_ref,
            frontend_url_root: data.frontend_url_root,
        })
    }
}
