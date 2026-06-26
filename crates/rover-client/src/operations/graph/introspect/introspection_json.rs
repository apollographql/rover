//! Convert introspected SDL into GraphQL introspection JSON (`{ "__schema": ... }`),
//! matching the legacy `apollo schema:download` / graphql-js `introspectionFromSchema` shape.

use apollo_compiler::{
    introspection, request::coerce_variable_values, response::JsonMap, ExecutableDocument, Schema,
};
use serde_json::Value;

use crate::error::RoverClientError;

const INTROSPECTION_QUERY: &str = include_str!("introspect_query.graphql");
const GRAPH_INTROSPECT_OPERATION: &str = "GraphIntrospectQuery";

/// Regenerate introspection JSON from SDL using apollo-compiler's introspection executor.
///
/// Returns a top-level `{ "__schema": { ... } }` object (no `data`/`errors` wrapper),
/// matching `JSON.stringify(introspectionFromSchema(schema))` from the legacy Apollo CLI.
pub fn sdl_to_introspection_json(sdl: &str) -> Result<Value, RoverClientError> {
    let schema = Schema::parse_and_validate(sdl, "introspection.graphql").map_err(|err| {
        RoverClientError::IntrospectionError {
            msg: err.errors.to_string(),
        }
    })?;

    let document = ExecutableDocument::parse_and_validate(
        &schema,
        INTROSPECTION_QUERY,
        "introspection_query.graphql",
    )
    .map_err(|err| RoverClientError::IntrospectionError {
        msg: err.errors.to_string(),
    })?;

    let operation = document
        .operations
        .get(Some(GRAPH_INTROSPECT_OPERATION))
        .map_err(|err| RoverClientError::IntrospectionError {
            msg: err.message().to_string(),
        })?;

    let variable_values =
        coerce_variable_values(&schema, operation, &JsonMap::default()).map_err(|err| {
            RoverClientError::IntrospectionError {
                msg: err.message().to_string(),
            }
        })?;

    let response = introspection::partial_execute(
        &schema,
        &schema.implementers_map(),
        &document,
        operation,
        &variable_values,
    )
    .map_err(|err| RoverClientError::IntrospectionError {
        msg: err.message().to_string(),
    })?;

    if !response.errors.is_empty() {
        let msg = response
            .errors
            .iter()
            .map(|err| err.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(RoverClientError::IntrospectionError {
            msg: format!("introspection query returned errors: {msg}"),
        });
    }

    let data = response
        .data
        .ok_or_else(|| RoverClientError::IntrospectionError {
            msg: "introspection query returned no data".to_string(),
        })?;

    serde_json::to_value(data).map_err(|err| RoverClientError::IntrospectionError {
        msg: err.to_string(),
    })
}

/// Decode `{ "__schema": ... }` introspection JSON to SDL and validate with apollo-compiler.
#[cfg(any(test, feature = "testing"))]
pub fn introspection_json_to_validated_sdl(
    introspection: &Value,
) -> Result<String, RoverClientError> {
    use std::convert::TryFrom;

    use apollo_compiler::Schema as CompilerSchema;

    use super::Schema as IntrospectionSchema;
    use crate::operations::graph::introspect::service::graph_introspect_query;

    let response_data: graph_introspect_query::ResponseData =
        serde_json::from_value(introspection.clone()).map_err(|err| {
            RoverClientError::IntrospectionError {
                msg: format!("failed to deserialize introspection JSON: {err}"),
            }
        })?;

    let schema = IntrospectionSchema::try_from(response_data).map_err(|msg| {
        RoverClientError::IntrospectionError {
            msg: msg.to_string(),
        }
    })?;

    let sdl = schema.encode();

    CompilerSchema::parse_and_validate(&sdl, "introspection.graphql").map_err(|err| {
        RoverClientError::IntrospectionError {
            msg: format!(
                "SDL from introspection JSON failed validation: {}",
                err.errors
            ),
        }
    })?;

    Ok(sdl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::graph::introspect::assert_structural_parity;

    const SIMPLE_SDL: &str = r#"
        type Query {
            hello: String
        }
    "#;

    #[test]
    fn top_level_shape() {
        let result = sdl_to_introspection_json(SIMPLE_SDL).unwrap();
        assert!(result.get("__schema").is_some());
        assert!(result.get("data").is_none());
        assert!(result.get("errors").is_none());

        let schema = &result["__schema"];
        assert!(schema.get("queryType").is_some());
        assert!(schema.get("types").is_some());
        assert!(schema.get("directives").is_some());

        let kinds: std::collections::BTreeSet<_> = schema["types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["kind"].as_str().unwrap().to_string())
            .collect();
        assert!(kinds.contains("OBJECT"));
        assert!(kinds.contains("SCALAR"));
        for kind in &kinds {
            assert_eq!(kind.as_str(), kind.to_uppercase());
        }
    }

    #[test]
    fn swapi_structural_parity_with_legacy_introspection_from_schema() {
        // Fixtures: see fixtures/README.md for source endpoint and regeneration steps.
        let sdl = include_str!("fixtures/swapi.graphql");
        let reference: Value =
            serde_json::from_str(include_str!("fixtures/swapi-introspection.json")).unwrap();

        let actual = sdl_to_introspection_json(sdl).unwrap();

        assert_structural_parity(&actual, &reference);
    }

    #[test]
    fn swapi_introspection_json_round_trips_through_sdl_to_valid_schema() {
        let sdl = include_str!("fixtures/swapi.graphql");
        let introspection = sdl_to_introspection_json(sdl).unwrap();

        introspection_json_to_validated_sdl(&introspection).unwrap();
    }

    #[test]
    fn swapi_reference_introspection_json_round_trips_through_sdl_to_valid_schema() {
        let reference: Value =
            serde_json::from_str(include_str!("fixtures/swapi-introspection.json")).unwrap();

        introspection_json_to_validated_sdl(&reference).unwrap();
    }
}
