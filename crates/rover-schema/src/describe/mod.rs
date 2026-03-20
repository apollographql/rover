pub mod deprecated;
pub mod schema_overview;
pub mod type_detail;
pub use schema_overview::SchemaOverview;
pub use type_detail::{
    ArgInfo, EnumDetail, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail, FieldInfo,
    FieldSummary, FieldsDetail, InputDetail, InterfaceDetail, ObjectDetail, ScalarDetail,
    TypeDetail, TypeKind, UnionDetail,
};

use apollo_compiler::Name;

impl crate::ParsedSchema {
    pub fn find_implementors(&self, interface_name: &Name) -> Vec<Name> {
        self.inner()
            .implementers_map()
            .get(interface_name)
            .map(|imp| imp.objects.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
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
        let coord: SchemaCoordinate = "User.posts".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.type_name.as_str()).is_equal_to("User");
        assert_that!(detail.field_name.as_str()).is_equal_to("posts");
        assert_that!(detail.arg_count).is_equal_to(2);
    }

    #[rstest]
    fn field_detail_not_found(test_schema: ParsedSchema) {
        let coord: SchemaCoordinate = "Post.nonExistent".parse().unwrap();
        assert_that!(test_schema.field_detail(&coord))
            .is_err()
            .matches(|e| {
                matches!(e, SchemaError::FieldNotFound { type_name, field }
                if type_name == "Post" && field == "nonExistent")
            });
    }

    #[rstest]
    fn field_detail_deprecated(test_schema: ParsedSchema) {
        let coord: SchemaCoordinate = "Post.oldSlug".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.is_deprecated).is_true();
        assert_that!(detail.deprecation_reason)
            .is_some()
            .is_equal_to("Use slug instead".to_string());
    }

    #[rstest]
    fn field_detail_expands_input_types(test_schema: ParsedSchema) {
        let coord: SchemaCoordinate = "Mutation.createPost".parse().unwrap();
        let detail = test_schema.field_detail(&coord);
        let detail = assert_that!(detail).is_ok().subject;
        assert_that!(detail.input_expansions).is_not_empty();
        assert_that!(detail.input_expansions).matching_contains(|t| t.name == "CreatePostInput");
    }
}
