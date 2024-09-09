use crate::error::RoverClientError;
use crate::operations::graph::introspect::types::*;
use crate::service::introspection::{
    IntrospectionConfig, IntrospectionQuery, IntrospectionService,
};

use graphql_client::GraphQLQuery;
use rover_http::HttpService;
use tower::Service;

use super::Schema;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_introspect_query
pub(crate) struct GraphIntrospectQuery;

impl IntrospectionQuery for GraphIntrospectQuery {
    type Response = GraphIntrospectResponse;
    fn variables() -> Self::Variables {
        graph_introspect_query::Variables {}
    }
    fn map_response(response_data: Self::ResponseData) -> Result<Self::Response, RoverClientError> {
        match Schema::try_from(response_data) {
            Ok(schema) => Ok(GraphIntrospectResponse {
                schema_sdl: schema.encode(),
            }),
            Err(msg) => Err(RoverClientError::IntrospectionError { msg: msg.into() }),
        }
    }
}

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub async fn run(
    config: IntrospectionConfig,
    http_service: HttpService,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    let mut introspection_service: IntrospectionService<GraphIntrospectQuery> =
        IntrospectionService::new(config, http_service);
    introspection_service.call(()).await
}
