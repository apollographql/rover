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

#[cfg(test)]
mod tests {
    use apollo_compiler::coord;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use crate::{ParsedSchema, SchemaError};

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("../../../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    #[rstest]
    fn returns_correct_coordinate_fields(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(limit:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.type_name.as_str()).is_equal_to("User");
        assert_that!(detail.field_name.as_str()).is_equal_to("posts");
        assert_that!(detail.arg_name.as_str()).is_equal_to("limit");
    }

    #[rstest]
    fn returns_correct_arg_type(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(limit:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.arg_type.inner_named_type().as_str()).is_equal_to("Int");
    }

    #[rstest]
    fn returns_description_when_present(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(limit:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.description.as_deref())
            .is_some()
            .is_equal_to("Maximum number of posts to return");
    }

    #[rstest]
    fn returns_none_description_when_absent(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(offset:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.description).is_none();
    }

    #[rstest]
    fn returns_default_value_when_present(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(limit:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.default_value.as_deref())
            .is_some()
            .is_equal_to("20");
    }

    #[rstest]
    fn returns_none_default_when_absent(schema: ParsedSchema) {
        let detail = schema.field_arg_detail(&coord!(User.posts(offset:)));
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.default_value).is_none();
    }

    #[rstest]
    fn errors_on_unknown_type(schema: ParsedSchema) {
        let err = schema.field_arg_detail(&coord!(Unknown.field(arg:)));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::TypeNotFound(_)));
    }

    #[rstest]
    fn errors_on_unknown_field(schema: ParsedSchema) {
        let err = schema.field_arg_detail(&coord!(User.nonexistent(arg:)));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::FieldNotFound { .. }));
    }

    #[rstest]
    fn errors_on_unknown_arg(schema: ParsedSchema) {
        let err = schema.field_arg_detail(&coord!(User.posts(nonexistent:)));
        assert_that!(err)
            .is_err()
            .matches(|e| matches!(e, SchemaError::FieldArgNotFound { .. }));
    }
}
