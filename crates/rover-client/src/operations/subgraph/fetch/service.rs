use std::{fmt, future::Future, pin::Pin};

use buildstructor::Builder;
use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    shared::{FetchResponse, GraphRef, Sdl, SdlType},
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_query
pub(crate) struct SubgraphFetchQuery;

impl fmt::Debug for subgraph_fetch_query::Variables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Variables")
            .field("graph_ref", &self.graph_ref)
            .field("subgraph_name", &self.subgraph_name)
            .finish()
    }
}

impl PartialEq for subgraph_fetch_query::Variables {
    fn eq(&self, other: &Self) -> bool {
        self.graph_ref == other.graph_ref && self.subgraph_name == other.subgraph_name
    }
}

#[derive(Builder)]
pub struct SubgraphFetchRequest {
    graph_ref: GraphRef,
    subgraph_name: String,
}

#[derive(Clone)]
pub struct SubgraphFetch<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphFetch<S> {
    pub fn new(inner: S) -> SubgraphFetch<S> {
        SubgraphFetch { inner }
    }
}

impl<S, Fut> Service<SubgraphFetchRequest> for SubgraphFetch<S>
where
    S: Service<
            GraphQLRequest<SubgraphFetchQuery>,
            Response = subgraph_fetch_query::ResponseData,
            Error = GraphQLServiceError<subgraph_fetch_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = FetchResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphFetchQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceError(Box::new(err)))
    }

    fn call(&mut self, req: SubgraphFetchRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let variables = subgraph_fetch_query::Variables {
                graph_ref: req.graph_ref.to_string(),
                subgraph_name: req.subgraph_name.to_string(),
            };
            let response_data = inner.call(GraphQLRequest::new(variables)).await?;
            get_sdl_from_response_data(req.graph_ref, req.subgraph_name, response_data)
        };
        Box::pin(fut)
    }
}

fn get_sdl_from_response_data(
    graph_ref: GraphRef,
    subgraph_name: String,
    response_data: subgraph_fetch_query::ResponseData,
) -> Result<FetchResponse, RoverClientError> {
    let subgraph = get_subgraph_from_response_data(graph_ref, subgraph_name, response_data)?;
    Ok(FetchResponse {
        sdl: Sdl {
            contents: subgraph.sdl,
            r#type: SdlType::Subgraph {
                routing_url: subgraph.url,
            },
        },
    })
}

#[derive(Debug, PartialEq)]
struct Subgraph {
    url: Option<String>,
    sdl: String,
}

fn get_subgraph_from_response_data(
    graph_ref: GraphRef,
    subgraph_name: String,
    response_data: subgraph_fetch_query::ResponseData,
) -> Result<Subgraph, RoverClientError> {
    if let Some(maybe_variant) = response_data.variant {
        match maybe_variant {
            subgraph_fetch_query::SubgraphFetchQueryVariant::GraphVariant(variant) => {
                if let Some(subgraph) = variant.subgraph {
                    Ok(Subgraph {
                        url: subgraph.url.clone(),
                        sdl: subgraph.active_partial_schema.sdl,
                    })
                } else if let Some(subgraphs) = variant.subgraphs {
                    let valid_subgraphs = subgraphs
                        .iter()
                        .map(|subgraph| subgraph.name.clone())
                        .collect();
                    Err(RoverClientError::NoSubgraphInGraph {
                        invalid_subgraph: subgraph_name,
                        valid_subgraphs,
                    })
                } else {
                    Err(RoverClientError::ExpectedFederatedGraph {
                        graph_ref,
                        can_operation_convert: true,
                    })
                }
            }
            _ => Err(RoverClientError::InvalidGraphRef),
        }
    } else {
        Err(RoverClientError::GraphNotFound { graph_ref })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::GraphRef;
    use rstest::{fixture, rstest};
    use serde_json::json;

    #[rstest]
    fn get_services_from_response_data_works(subgraph_name: String, graph_ref: GraphRef) {
        let sdl = "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            .to_string();
        let url = "http://my.subgraph.com".to_string();
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraphs": [
                    { "name": "accounts" },
                    { "name": &subgraph_name }
                ],
                "subgraph": {
                    "url": &url,
                    "activePartialSchema": {
                        "sdl": &sdl
                    }
                }
            }
        });
        let data: subgraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let expected_subgraph = Subgraph {
            url: Some(url),
            sdl,
        };
        let output = get_subgraph_from_response_data(graph_ref, subgraph_name, data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_subgraph);
    }

    #[rstest]
    fn get_services_from_response_data_errs_with_no_variant(
        subgraph_name: String,
        graph_ref: GraphRef,
    ) {
        let json_response = json!({ "variant": null });
        let data: subgraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_subgraph_from_response_data(graph_ref, subgraph_name, data);
        assert!(output.is_err());
    }

    #[rstest]
    fn get_sdl_for_service_errs_on_invalid_name(subgraph_name: String, graph_ref: GraphRef) {
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraphs": [
                    { "name": "accounts" },
                    { "name": &subgraph_name }
                ],
                "subgraph": null
            }
        });
        let data: subgraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_subgraph_from_response_data(graph_ref, subgraph_name, data);

        assert!(output.is_err());
    }

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }

    #[fixture]
    fn subgraph_name() -> String {
        "products".to_string()
    }
}
