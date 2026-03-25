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
