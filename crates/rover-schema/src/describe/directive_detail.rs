use apollo_compiler::{Name, ast::DirectiveDefinition};

use crate::{ParsedSchema, SchemaError, describe::type_detail::ArgInfo};

/// Detailed view of a GraphQL directive definition.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectiveDetail {
    /// The directive name (without `@`).
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Arguments accepted by this directive.
    pub args: Vec<ArgInfo>,
    /// Locations where this directive may be applied.
    pub locations: Vec<String>,
    /// Whether the directive may be applied more than once at a location.
    pub repeatable: bool,
}

/// Detailed view of a single argument on a directive definition.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectiveArgDetail {
    /// The directive name (without `@`).
    pub directive_name: Name,
    /// The argument name.
    pub arg_name: Name,
    /// The inner named type of the argument.
    pub arg_type: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// The default value as a string, if one is specified.
    pub default_value: Option<String>,
}

impl ParsedSchema {
    /// Return detail for the directive identified by `name`.
    pub fn directive_detail(&self, name: &Name) -> Result<DirectiveDetail, SchemaError> {
        let def = self
            .inner()
            .directive_definitions
            .get(name)
            .ok_or_else(|| SchemaError::DirectiveNotFound(name.clone()))?;

        Ok(build_directive_detail(name.clone(), def))
    }

    /// Return detail for the argument `arg_name` on the directive `directive_name`.
    pub fn directive_arg_detail(
        &self,
        directive_name: &Name,
        arg_name: &Name,
    ) -> Result<DirectiveArgDetail, SchemaError> {
        let def = self
            .inner()
            .directive_definitions
            .get(directive_name)
            .ok_or_else(|| SchemaError::DirectiveNotFound(directive_name.clone()))?;

        let arg = def
            .arguments
            .iter()
            .find(|a| a.name == *arg_name)
            .ok_or_else(|| SchemaError::DirectiveArgNotFound {
                directive: directive_name.clone(),
                argument: arg_name.clone(),
            })?;

        Ok(DirectiveArgDetail {
            directive_name: directive_name.clone(),
            arg_name: arg.name.clone(),
            arg_type: arg.ty.inner_named_type().clone(),
            description: arg.description.as_ref().map(|d| d.to_string()),
            default_value: arg.default_value.as_ref().map(|v| v.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::name;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use crate::{ParsedSchema, SchemaError};

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    // --- directive_detail ---

    #[rstest]
    fn returns_name_and_description(schema: ParsedSchema) {
        let detail = schema.directive_detail(&name!("auth"));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.name.as_str()).is_equal_to("auth");
        assert_that!(detail.description.as_deref())
            .is_some()
            .is_equal_to("Marks a field or object as requiring a minimum role");
    }

    #[rstest]
    fn returns_locations(schema: ParsedSchema) {
        let detail = schema.directive_detail(&name!("auth"));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.locations).contains("FIELD_DEFINITION".to_string());
        assert_that!(detail.locations).contains("OBJECT".to_string());
    }

    #[rstest]
    fn returns_repeatable_false_for_non_repeatable(schema: ParsedSchema) {
        let detail = schema.directive_detail(&name!("auth"));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.repeatable).is_false();
    }

    #[rstest]
    fn returns_args(schema: ParsedSchema) {
        let detail = schema.directive_detail(&name!("auth"));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.args).has_length(1);
        let arg = &detail.args[0];
        assert_that!(arg.name.as_str()).is_equal_to("requires");
        assert_that!(arg.arg_type.as_str()).is_equal_to("Role");
        assert_that!(arg.description.as_deref())
            .is_some()
            .is_equal_to("The minimum role required to access this field");
        assert_that!(arg.default_value.as_deref())
            .is_some()
            .is_equal_to("USER");
    }

    #[rstest]
    fn errors_on_unknown_directive(schema: ParsedSchema) {
        let err = schema.directive_detail(&name!("nonexistent"));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::DirectiveNotFound(_)));
    }

    // --- directive_arg_detail ---

    #[rstest]
    fn arg_detail_returns_correct_fields(schema: ParsedSchema) {
        let detail = schema.directive_arg_detail(&name!("auth"), &name!("requires"));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.directive_name.as_str()).is_equal_to("auth");
        assert_that!(detail.arg_name.as_str()).is_equal_to("requires");
        assert_that!(detail.arg_type.as_str()).is_equal_to("Role");
        assert_that!(detail.description.as_deref())
            .is_some()
            .is_equal_to("The minimum role required to access this field");
        assert_that!(detail.default_value.as_deref())
            .is_some()
            .is_equal_to("USER");
    }

    #[rstest]
    fn arg_detail_errors_on_unknown_directive(schema: ParsedSchema) {
        let err = schema.directive_arg_detail(&name!("nonexistent"), &name!("requires"));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::DirectiveNotFound(_)));
    }

    #[rstest]
    fn arg_detail_errors_on_unknown_arg(schema: ParsedSchema) {
        let err = schema.directive_arg_detail(&name!("auth"), &name!("nonexistent"));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::DirectiveArgNotFound { .. }));
    }
}

fn build_directive_detail(name: Name, def: &DirectiveDefinition) -> DirectiveDetail {
    let description = def.description.as_ref().map(|d| d.to_string());
    let args = def
        .arguments
        .iter()
        .map(|arg| ArgInfo {
            name: arg.name.clone(),
            arg_type: arg.ty.inner_named_type().clone(),
            description: arg.description.as_ref().map(|d| d.to_string()),
            default_value: arg.default_value.as_ref().map(|v| v.to_string()),
        })
        .collect();
    let locations = def.locations.iter().map(|l| l.to_string()).collect();
    DirectiveDetail {
        name,
        description,
        args,
        locations,
        repeatable: def.repeatable,
    }
}
