use std::{fmt, future::Future, pin::Pin, str::FromStr};

use apollo_federation_types::config::{FederationVersion, SchemaSource, SubgraphConfig};
use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{shared::GraphRef, EndpointKind, RoverClientError};

use super::{types::Subgraph, SubgraphFetchAllResponse};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/fetch_all/fetch_all_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_all_query
pub struct SubgraphFetchAllQuery;

impl fmt::Debug for subgraph_fetch_all_query::Variables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Variables")
            .field("graph_ref", &self.graph_ref)
            .finish()
    }
}

impl PartialEq for subgraph_fetch_all_query::Variables {
    fn eq(&self, other: &Self) -> bool {
        self.graph_ref == other.graph_ref
    }
}

pub struct SubgraphFetchAllRequest {
    graph_ref: GraphRef,
}

impl SubgraphFetchAllRequest {
    pub fn new(graph_ref: GraphRef) -> SubgraphFetchAllRequest {
        SubgraphFetchAllRequest { graph_ref }
    }
}

#[derive(Clone)]
pub struct SubgraphFetchAll<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphFetchAll<S> {
    pub fn new(inner: S) -> SubgraphFetchAll<S> {
        SubgraphFetchAll { inner }
    }
}

impl<S, Fut> Service<SubgraphFetchAllRequest> for SubgraphFetchAll<S>
where
    S: Service<
            GraphQLRequest<SubgraphFetchAllQuery>,
            Response = subgraph_fetch_all_query::ResponseData,
            Error = GraphQLServiceError<subgraph_fetch_all_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = SubgraphFetchAllResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphFetchAllQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: SubgraphFetchAllRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let variables = subgraph_fetch_all_query::Variables {
                graph_ref: req.graph_ref.to_string(),
            };
            inner
                .call(GraphQLRequest::<SubgraphFetchAllQuery>::new(variables))
                .await
                //.map_err(|err| RoverClientError::Service {
                    //source: Box::new(err),
                    //endpoint_kind: EndpointKind::ApolloStudio,
                //})
                .map_err(|err| {
                    match err {
                        GraphQLServiceError::InvalidCredentials() => {
                            RoverClientError::PermissionError {
                                msg: "attempting to fetch subgraphs".to_string(),
                            }
                        }
                        _ => {
                            RoverClientError::Service {
                                source: Box::new(err),
                                endpoint_kind: EndpointKind::ApolloStudio,
                            }
                        }
                    }
                })
                .and_then(|response_data| {
                    get_subgraphs_from_response_data(req.graph_ref, response_data)
                })
        };
        Box::pin(fut)
    }
}

fn get_subgraphs_from_response_data(
    graph_ref: GraphRef,
    response_data: subgraph_fetch_all_query::ResponseData,
) -> Result<SubgraphFetchAllResponse, RoverClientError> {
    match response_data.variant {
        None => Err(RoverClientError::GraphNotFound { graph_ref }),
        Some(subgraph_fetch_all_query::SubgraphFetchAllQueryVariant::GraphVariant(variant)) => {
            extract_subgraphs_from_response(variant, graph_ref)
        }
        _ => Err(RoverClientError::InvalidGraphRef),
    }
}
fn extract_subgraphs_from_response(
    value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariant,
    graph_ref: GraphRef,
) -> Result<SubgraphFetchAllResponse, RoverClientError> {
    match (value.subgraphs, value.source_variant) {
        // If we get null back in both branches or the query, or we get a structure in the
        // sourceVariant half but there are no subgraphs in it. Then we return an error
        // because this isn't a FederatedSubgraph **as far as we can tell**.
        (None, None)
        | (
            None,
            Some(
                subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariant {
                    subgraphs: None,
                    ..
                },
            ),
        ) => Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: true,
        }),
        // If we get nothing from querying the subgraphs directly, but we do get some subgraphs
        // on the sourceVariant side of the query, we just return those.
        (
            None,
            Some(
                subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariant {
                    subgraphs: Some(subgraphs),
                    latest_launch,
                },
            ),
        ) => Ok(SubgraphFetchAllResponse {
            subgraphs: subgraphs
                .into_iter()
                .map(|subgraph| subgraph.into())
                .collect(),
            federation_version: latest_launch.and_then(|it| it.into()),
        }),
        // Here there are three cases where we might want to return the subgraphs we got from
        // directly querying the graphVariant:
        // 1. If we get subgraphs back from the graphVariant directly and nothing from the sourceVariant
        // 2. If we get subgraphs back from the graphVariant directly and a structure from the
        // sourceVariant, but it contains no subgraphs
        // 3. If we get subgraphs back from both 'sides' of the query, we take the results from
        // querying the **graphVariant**, as this is closest to the original behaviour, before
        // we introduced the querying of the sourceVariant.
        (Some(subgraphs), _) => Ok(SubgraphFetchAllResponse {
            subgraphs: subgraphs
                .into_iter()
                .map(|subgraph| subgraph.into())
                .collect(),
            federation_version: value.latest_launch.and_then(|it| it.into()),
        }),
    }
}

impl From<Subgraph> for SubgraphConfig {
    fn from(value: Subgraph) -> Self {
        Self {
            routing_url: value.url().clone(),
            schema: SchemaSource::Sdl {
                sdl: value.sdl().clone(),
            },
        }
    }
}

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSubgraphs>
    for Subgraph
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSubgraphs,
    ) -> Self {
        Subgraph::builder()
            .name(value.name)
            .and_url(value.url)
            .sdl(value.active_partial_schema.sdl)
            .build()
    }
}

impl
    From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantSubgraphs>
    for Subgraph
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantSubgraphs,
    ) -> Self {
        Subgraph::builder()
            .name(value.name)
            .and_url(value.url)
            .sdl(value.active_partial_schema.sdl)
            .build()
    }
}

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunch>
    for Option<FederationVersion>
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunch,
    ) -> Self {
        if let subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunchBuildInput::CompositionBuildInput(composition_build_input) = value.build_input {
            composition_build_input
                .version
                .as_ref()
                .and_then(|v| FederationVersion::from_str(&("=".to_owned() + v)).ok())
        } else {
            None
        }
    }
}

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunch>
    for Option<FederationVersion>
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunch,
    ) -> Self {
        if let subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunchBuildInput::CompositionBuildInput(composition_build_input) = value.build_input {
            composition_build_input.version.as_ref().and_then(|v| FederationVersion::from_str(&("=".to_owned() + v)).ok())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::config::FederationVersion;
    use rstest::{fixture, rstest};
    use semver::Version;
    use serde_json::{json, Value};

    use crate::shared::GraphRef;

    use super::*;

    const SDL: &str =
        "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n";
    const URL: &str = "http://my.subgraph.com";
    const SUBGRAPH_NAME: &str = "accounts";

    #[rstest]
    #[case::subgraphs_returned_direct_from_variant(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": [
                 {
                    "name": SUBGRAPH_NAME,
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                     }
                },
            ],
            "latestLaunch": {
                "buildInput": {
                    "__typename": "CompositionBuildInput",
                    "version": "2.3.4"
                }
            },
            "sourceVariant": null
        }
    }), Some(SubgraphFetchAllResponse {
        subgraphs: vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()],
        federation_version: Some(FederationVersion::ExactFedTwo(Version::new(2, 3, 4))),
    }))]
    #[case::subgraphs_returned_via_source_variant(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": null,
            "sourceVariant": {
                "subgraphs": [
                {
                    "name": SUBGRAPH_NAME,
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                    }
                }
                ],
                "latestLaunch": {
                    "buildInput": {
                        "__typename": "CompositionBuildInput",
                        "version": "2.3.4"
                    }
                }
            }
        }
    }), Some(SubgraphFetchAllResponse {
        subgraphs: vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()],
        federation_version: Some(FederationVersion::ExactFedTwo(Version::new(2, 3, 4))),
    }))]
    #[case::no_subgraphs_returned_in_either_case(json!(
    {
        "variant": {
            "__typename": "GraphVariant",
            "subgraphs": null,
            "sourceVariant": {
                "subgraphs": null
            }
        }
    }), None)]
    #[case::subgraphs_returned_from_both_sides_of_the_query_means_we_get_the_variants_subgraphs(json!(
    {
        "variant": {
        "__typename": "GraphVariant",
        "subgraphs": [
            {
                "name": SUBGRAPH_NAME,
                "url": URL,
                "activePartialSchema": {
                    "sdl": SDL
                }
            }
        ],
        "latestLaunch": {
            "buildInput": {
                "__typename": "CompositionBuildInput",
                "version": "2.3.4"
            }
        },
        "sourceVariant": {
            "subgraphs": [
                {
                    "name": "banana",
                    "url": URL,
                    "activePartialSchema": {
                        "sdl": SDL
                    }
                 }
             ],
             "latestLaunch": {
                "buildInput": {
                    "__typename": "CompositionBuildInput",
                    "version": "2.9.9"
                }
            }
        }
    }
    }), Some(SubgraphFetchAllResponse {
        subgraphs: vec![Subgraph::builder().url(URL).sdl(SDL).name(SUBGRAPH_NAME).build()],
        federation_version: Some(FederationVersion::ExactFedTwo(Version::new(2, 3, 4))),
    }))]
    fn get_services_from_response_data_works(
        graph_ref: GraphRef,
        #[case] json_response: Value,
        #[case] expected_subgraphs: Option<SubgraphFetchAllResponse>,
    ) {
        let data: subgraph_fetch_all_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(graph_ref, data);

        if expected_subgraphs.is_some() {
            assert!(output.is_ok());
            assert_eq!(output.unwrap(), expected_subgraphs.unwrap());
        } else {
            assert!(output.is_err());
        };
    }

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
