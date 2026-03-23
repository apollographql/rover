/// Deprecated field/value detection helpers.
pub mod deprecated;
/// High-level schema overview stats.
pub mod schema_overview;
/// Per-type detail views.
pub mod type_detail;
use apollo_compiler::coordinate::SchemaCoordinate;
pub use schema_overview::SchemaOverview;
pub use type_detail::{
    ArgInfo, EnumDetail, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail, FieldInfo,
    FieldSummary, FieldsDetail, InputDetail, InputFieldInfo, InterfaceDetail, ObjectDetail,
    ScalarDetail, TypeDetail, UnionDetail,
};

use crate::error::SchemaError;

/// The result of a `describe` operation, which varies by the coordinate provided.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum DescribeOutput {
    /// A high-level summary of the entire schema.
    Overview(SchemaOverview),
    /// Detail for a specific named type.
    Type(TypeDetail),
    /// Detail for a specific field on a type.
    Field(FieldDetail),
}

impl crate::ParsedSchema {
    /// Describe the schema or a specific coordinate within it.
    ///
    /// Pass `None` for `coord` to get a full schema overview. Pass a type or
    /// field coordinate to get detail for that specific item.
    pub fn describe(
        &self,
        coord: Option<&SchemaCoordinate>,
        schema_source: String,
        include_deprecated: bool,
        depth: usize,
    ) -> Result<DescribeOutput, SchemaError> {
        match coord {
            None => Ok(DescribeOutput::Overview(self.overview(schema_source))),
            Some(SchemaCoordinate::Type(tc)) => self
                .type_detail(&tc.ty, include_deprecated, depth)
                .map(DescribeOutput::Type),
            Some(SchemaCoordinate::TypeAttribute(tac)) => {
                self.field_detail(tac).map(DescribeOutput::Field)
            }
            // TODO: directives
            Some(other) => Err(SchemaError::UnsupportedCoordinate(other.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::TypeAttributeCoordinate;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use crate::{ParsedSchema, SchemaError};

    #[fixture]
    fn test_schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[rstest]
    fn field_detail_with_args(test_schema: ParsedSchema) {
        let coord: TypeAttributeCoordinate = "User.posts".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.type_name.as_str()).is_equal_to("User");
        assert_that!(detail.field_name.as_str()).is_equal_to("posts");
        assert_that!(detail.arg_count).is_equal_to(2);
    }

    #[rstest]
    fn field_detail_not_found(test_schema: ParsedSchema) {
        let coord: TypeAttributeCoordinate = "Post.nonExistent".parse().unwrap();
        assert_that!(test_schema.field_detail(&coord))
            .is_err()
            .matches(|e| {
                matches!(e, SchemaError::FieldNotFound { type_name, field }
                if type_name == "Post" && field == "nonExistent")
            });
    }

    #[rstest]
    fn field_detail_deprecated(test_schema: ParsedSchema) {
        let coord: TypeAttributeCoordinate = "Post.oldSlug".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.is_deprecated).is_true();
        assert_that!(detail.deprecation_reason)
            .is_some()
            .is_equal_to("Use slug instead".to_string());
    }

    #[rstest]
    fn field_detail_expands_input_types(test_schema: ParsedSchema) {
        let coord: TypeAttributeCoordinate = "Mutation.createPost".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.input_expansions).is_not_empty();
        assert_that!(detail.input_expansions).matching_contains(|t| t.name() == "CreatePostInput");
    }
}
