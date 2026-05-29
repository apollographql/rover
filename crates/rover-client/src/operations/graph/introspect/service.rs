use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use serde_json::Value;
use tower::Service;

use crate::{EndpointKind, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct GraphIntrospectQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query_legacy.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct GraphIntrospectLegacyQuery;

#[derive(thiserror::Error, Debug)]
pub enum GraphIntrospectError {
    #[error("Inner service failed to become ready.\n{}", .0)]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    ModernGraphQL(GraphQLServiceError<graph_introspect_query::ResponseData>),
    #[error(transparent)]
    LegacyGraphQL(GraphQLServiceError<graph_introspect_legacy_query::ResponseData>),
    #[error("Failed to convert legacy introspection response: {0}")]
    LegacyConversion(serde_json::Error),
}

impl From<GraphIntrospectError> for RoverClientError {
    fn from(value: GraphIntrospectError) -> Self {
        match value {
            GraphIntrospectError::ServiceReady(err) => RoverClientError::ServiceReady(err),
            GraphIntrospectError::ModernGraphQL(err) => RoverClientError::Service {
                source: Box::new(err),
                endpoint_kind: EndpointKind::Customer,
            },
            GraphIntrospectError::LegacyGraphQL(err) => RoverClientError::Service {
                source: Box::new(err),
                endpoint_kind: EndpointKind::Customer,
            },
            GraphIntrospectError::LegacyConversion(err) => RoverClientError::IntrospectionError {
                msg: format!("failed to convert legacy introspection response: {err}"),
            },
        }
    }
}

#[derive(Clone)]
pub struct GraphIntrospect<S: Clone> {
    inner: S,
}

impl<S: Clone> GraphIntrospect<S> {
    pub const fn new(inner: S) -> GraphIntrospect<S> {
        GraphIntrospect { inner }
    }
}

impl<S, Fut> Service<()> for GraphIntrospect<S>
where
    S: Service<
            GraphQLRequest<GraphIntrospectQuery>,
            Response = graph_introspect_query::ResponseData,
            Error = GraphQLServiceError<graph_introspect_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = graph_introspect_query::ResponseData;
    type Error = GraphIntrospectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::<GraphQLRequest<GraphIntrospectQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| GraphIntrospectError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            inner
                .call(GraphQLRequest::<GraphIntrospectQuery>::new(
                    graph_introspect_query::Variables {},
                ))
                .await
                .map_err(GraphIntrospectError::ModernGraphQL)
        };
        Box::pin(fut)
    }
}

#[derive(Clone)]
pub struct GraphIntrospectLegacy<S: Clone> {
    inner: S,
}

impl<S: Clone> GraphIntrospectLegacy<S> {
    pub const fn new(inner: S) -> GraphIntrospectLegacy<S> {
        GraphIntrospectLegacy { inner }
    }
}

impl<S, Fut> Service<()> for GraphIntrospectLegacy<S>
where
    S: Service<
            GraphQLRequest<GraphIntrospectLegacyQuery>,
            Response = graph_introspect_legacy_query::ResponseData,
            Error = GraphQLServiceError<graph_introspect_legacy_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = graph_introspect_query::ResponseData;
    type Error = GraphIntrospectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::<GraphQLRequest<GraphIntrospectLegacyQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| GraphIntrospectError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let legacy_data = inner
                .call(GraphQLRequest::<GraphIntrospectLegacyQuery>::new(
                    graph_introspect_legacy_query::Variables {},
                ))
                .await
                .map_err(GraphIntrospectError::LegacyGraphQL)?;
            legacy_response_to_query_response(legacy_data)
                .map_err(GraphIntrospectError::LegacyConversion)
        };
        Box::pin(fut)
    }
}

/// Convert the legacy introspection response into the shape expected by
/// `Schema`. The legacy query omits `isDeprecated`/`deprecationReason` on
/// `__InputValue`, so we serialize through JSON and inject the defaults
/// (`false` / `null`) at the three locations where `__InputValue` appears
/// in the response: `types[].fields[].args[]`, `types[].inputFields[]`,
/// and `directives[].args[]`.
fn legacy_response_to_query_response(
    legacy: graph_introspect_legacy_query::ResponseData,
) -> Result<graph_introspect_query::ResponseData, serde_json::Error> {
    let mut json = serde_json::to_value(&legacy)?;
    patch_legacy_input_values(&mut json);
    serde_json::from_value(json)
}

fn patch_legacy_input_values(value: &mut Value) {
    let Some(schema) = value.get_mut("schema") else {
        return;
    };

    for_each_in_array(schema, "types", |typ| {
        for_each_in_array(typ, "fields", |field| {
            for_each_in_array(field, "args", inject_input_value_defaults);
        });
        for_each_in_array(typ, "inputFields", inject_input_value_defaults);
    });

    for_each_in_array(schema, "directives", |dir| {
        for_each_in_array(dir, "args", inject_input_value_defaults);
    });
}

fn for_each_in_array(parent: &mut Value, key: &str, mut f: impl FnMut(&mut Value)) {
    if let Some(arr) = parent.get_mut(key).and_then(Value::as_array_mut) {
        arr.iter_mut().for_each(&mut f);
    }
}

fn inject_input_value_defaults(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.entry("isDeprecated").or_insert(Value::Bool(false));
    obj.entry("deprecationReason").or_insert(Value::Null);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn patcher_fills_input_value_defaults_under_all_paths() {
        let mut value = json!({
            "schema": {
                "types": [{
                    "fields": [{ "args": [{ "name": "a" }] }],
                    "inputFields": [{ "name": "b" }],
                }],
                "directives": [{ "args": [{ "name": "c" }] }],
            }
        });
        patch_legacy_input_values(&mut value);

        let arg = &value["schema"]["types"][0]["fields"][0]["args"][0];
        assert_eq!(arg["isDeprecated"], json!(false));
        assert_eq!(arg["deprecationReason"], json!(null));

        let input_field = &value["schema"]["types"][0]["inputFields"][0];
        assert_eq!(input_field["isDeprecated"], json!(false));
        assert_eq!(input_field["deprecationReason"], json!(null));

        let directive_arg = &value["schema"]["directives"][0]["args"][0];
        assert_eq!(directive_arg["isDeprecated"], json!(false));
        assert_eq!(directive_arg["deprecationReason"], json!(null));
    }

    #[test]
    fn patcher_preserves_existing_deprecation_values() {
        let mut value = json!({
            "schema": {
                "types": [{
                    "inputFields": [{
                        "name": "b",
                        "isDeprecated": true,
                        "deprecationReason": "old",
                    }],
                }],
            }
        });
        patch_legacy_input_values(&mut value);
        let f = &value["schema"]["types"][0]["inputFields"][0];
        assert_eq!(f["isDeprecated"], json!(true));
        assert_eq!(f["deprecationReason"], json!("old"));
    }
}
