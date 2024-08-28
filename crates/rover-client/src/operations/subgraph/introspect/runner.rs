use rover_http::HttpService;
use tower::Service;

use crate::operations::subgraph::introspect::types::*;
use crate::service::introspection::{
    IntrospectionConfig, IntrospectionQuery, IntrospectionService,
};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/introspect/introspect_query.graphql",
    schema_path = "src/operations/subgraph/introspect/introspect_schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct SubgraphIntrospectQuery;

impl IntrospectionQuery for SubgraphIntrospectQuery {
    type Response = SubgraphIntrospectResponse;

    fn variables() -> Self::Variables {
        Self::Variables {}
    }

    fn map_response(response_data: Self::ResponseData) -> Result<Self::Response, RoverClientError> {
        let graph = response_data
            .service
            .ok_or(RoverClientError::IntrospectionError {
                msg: "No introspection response available.".to_string(),
            })?;
        Ok(SubgraphIntrospectResponse { result: graph.sdl })
    }
}

pub async fn run(
    config: IntrospectionConfig,
    http_service: HttpService,
) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let mut introspection_service: IntrospectionService<SubgraphIntrospectQuery> =
        IntrospectionService::new(config, http_service);
    introspection_service.call(()).await
}
