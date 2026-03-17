use std::future::Future;

use apollo_federation_types::rover::BuildError;
use futures::future::TryFutureExt;
use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::ResponseFuture;
use tower::Service;

use crate::{
    shared::{FetchResponse, GraphRef, Sdl, SdlType},
    RoverClientError,
};

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/supergraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. supergraph_fetch_query
pub struct SupergraphFetchQuery;

pub struct SupergraphFetchRequest {
    graph_ref: GraphRef,
}

impl SupergraphFetchRequest {
    pub fn new(graph_ref: GraphRef) -> SupergraphFetchRequest {
        SupergraphFetchRequest { graph_ref }
    }
}

impl From<GraphRef> for SupergraphFetchRequest {
    fn from(value: GraphRef) -> Self {
        SupergraphFetchRequest::new(value)
    }
}

pub struct SupergraphFetch<S> {
    inner: S,
}

impl<S> SupergraphFetch<S> {
    pub const fn new(inner: S) -> SupergraphFetch<S> {
        SupergraphFetch { inner }
    }
}

impl<S, Fut> Service<SupergraphFetchRequest> for SupergraphFetch<S>
where
    S: Service<
            GraphQLRequest<SupergraphFetchQuery>,
            Response = supergraph_fetch_query::ResponseData,
            Error = GraphQLServiceError<supergraph_fetch_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send + 'static,
{
    type Response = FetchResponse;
    type Error = RoverClientError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SupergraphFetchQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: SupergraphFetchRequest) -> Self::Future {
        let variables = supergraph_fetch_query::Variables {
            graph_id: req.graph_ref.name.clone(),
            variant: req.graph_ref.variant.clone(),
        };
        let graphql_request = GraphQLRequest::new(variables);
        let fut = self
            .inner
            .call(graphql_request)
            .map_err(RoverClientError::from)
            .and_then(
                |resp| async move { get_supergraph_sdl_from_response_data(resp, req.graph_ref) },
            );
        Box::pin(fut)
    }
}

pub(crate) fn get_supergraph_sdl_from_response_data(
    response_data: supergraph_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<FetchResponse, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    if let Some(result) = graph
        .variant
        .and_then(|x| x.latest_approved_launch)
        .and_then(|x| x.build)
        .and_then(|x| x.result)
    {
        match result {
            supergraph_fetch_query::SupergraphFetchQueryGraphVariantLatestApprovedLaunchBuildResult::BuildFailure(failure) =>
                Err(RoverClientError::NoSupergraphBuilds {
                    graph_ref,
                    source: failure
                        .error_messages
                        .into_iter()
                        .map(|error| BuildError::composition_error(error.code, Some(error.message), None, None))
                        .collect(),
                }),
            supergraph_fetch_query::SupergraphFetchQueryGraphVariantLatestApprovedLaunchBuildResult::BuildSuccess(success) =>
                Ok(FetchResponse {
                    sdl: Sdl {
                        contents: success.core_schema.core_document,
                        r#type: SdlType::Supergraph,
                    },
                })
        }
    } else {
        let valid_variants = graph
            .variants
            .into_iter()
            .map(|v| v.name)
            .collect::<Vec<_>>();

        if !valid_variants.contains(&graph_ref.variant) {
            Err(RoverClientError::NoSchemaForVariant {
                graph_ref,
                valid_variants,
                frontend_url_root: response_data.frontend_url_root,
            })
        } else {
            Err(RoverClientError::ExpectedFederatedGraph {
                graph_ref,
                can_operation_convert: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::rover::BuildErrors;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[rstest]
    fn get_supergraph_sdl_from_response_data_works(graph_ref: GraphRef) {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "latestApprovedLaunch": {
                        "build": {
                            "result": {
                                "__typename": "BuildSuccess",
                                "coreSchema": {
                                    "coreDocument": "type Query { hello: String }",
                                },
                            },
                        },
                    },
                },
                "variants": [],
                "mostRecentCompositionPublish": {
                    "errors": []
                }
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref);

        assert_that!(output).is_ok().is_equal_to(FetchResponse {
            sdl: Sdl {
                contents: "type Query { hello: String }".to_string(),
                r#type: SdlType::Supergraph,
            },
        });
    }

    #[rstest]
    fn get_schema_from_response_data_errs_on_no_graph(graph_ref: GraphRef) {
        let json_response =
            json!({ "graph": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        assert_that!(output).is_err().matches(|err| {
            if let RoverClientError::GraphNotFound {
                graph_ref: actual_graph_ref,
            } = err
            {
                &graph_ref == actual_graph_ref
            } else {
                false
            }
        });
    }

    #[rstest]
    fn get_schema_from_response_data_errs_on_invalid_variant(graph_ref: GraphRef) {
        let valid_variant = "cccuuurrreeennnttt".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "graph": {
                "variant": null,
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionPublish": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        assert_that!(output).is_err().matches(|err| {
            if let RoverClientError::NoSchemaForVariant {
                graph_ref: actual_graph_ref,
                valid_variants: actual_valid_variants,
                frontend_url_root: actual_frontend_url_root,
            } = err
            {
                &graph_ref == actual_graph_ref
                    && &vec![valid_variant.clone()] == actual_valid_variants
                    && &frontend_url_root == actual_frontend_url_root
            } else {
                false
            }
        });
    }

    #[rstest]
    fn get_schema_from_response_data_errs_on_build_failure(graph_ref: GraphRef) {
        let valid_variant = "current".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "graph": {
                "variant": {
                    "latestApprovedLaunch": {
                        "build": {
                            "result": {
                                "__typename": "BuildFailure",
                                "errorMessages": []
                            }
                        }
                    }
                },
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionResult": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        assert_that!(output).is_err().matches(|err| {
            if let RoverClientError::NoSupergraphBuilds {
                graph_ref: actual_graph_ref,
                source: actual_source,
            } = err
            {
                &graph_ref == actual_graph_ref && &BuildErrors::new() == actual_source
            } else {
                false
            }
        });
    }
}
