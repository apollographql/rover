use apollo_compiler::{
    Name,
    ast::Type as AstType,
    coordinate::{FieldArgumentCoordinate, SchemaLookupError},
};

use crate::{ParsedSchema, SchemaError};

/// Detailed view of a single argument on a type's field.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldArgDetail {
    /// The name of the type that owns the field.
    pub type_name: Name,
    /// The field name.
    pub field_name: Name,
    /// The argument name.
    pub arg_name: Name,
    /// The full argument type.
    pub arg_type: AstType,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// The default value as a string, if one is specified.
    pub default_value: Option<String>,
}

impl ParsedSchema {
    /// Return detail for the argument identified by `coord`.
    pub fn field_arg_detail(
        &self,
        coord: &FieldArgumentCoordinate,
    ) -> Result<FieldArgDetail, SchemaError> {
        let arg = coord.lookup(self.inner()).map_err(|e| match e {
            SchemaLookupError::MissingType(_) => SchemaError::TypeNotFound(coord.ty.clone()),
            SchemaLookupError::MissingAttribute(_)
            | SchemaLookupError::InvalidArgumentAttribute(_) => SchemaError::FieldNotFound {
                type_name: coord.ty.clone(),
                field: coord.field.clone(),
            },
            _ => SchemaError::FieldArgNotFound {
                type_name: coord.ty.clone(),
                field: coord.field.clone(),
                argument: coord.argument.clone(),
            },
        })?;

        Ok(FieldArgDetail {
            type_name: coord.ty.clone(),
            field_name: coord.field.clone(),
            arg_name: arg.name.clone(),
            arg_type: (*arg.ty).clone(),
            description: arg.description.as_ref().map(|d| d.to_string()),
            default_value: arg.default_value.as_ref().map(|v| v.to_string()),
        })
    }
}
