pub mod deprecated;
pub mod schema_overview;
pub mod type_detail;
pub use schema_overview::SchemaOverview;
pub use type_detail::{
    ArgInfo, EnumDetail, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail,
    FieldInfo, FieldSummary, FieldsDetail, InputDetail, InterfaceDetail, ObjectDetail,
    ScalarDetail, TypeDetail, TypeKind, UnionDetail,
};

use apollo_compiler::Name;

/// Result of describing a schema at different levels of detail.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum DescribeResult {
    Overview(SchemaOverview),
    TypeDetail(TypeDetail),
    FieldDetail(FieldDetail),
}

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

    use crate::ParsedSchema;

    fn test_schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[test]
    fn field_detail_with_args() {
        let schema = test_schema();
        let coord: SchemaCoordinate = "User.posts".parse().unwrap();
        let detail = schema.field_detail(&coord).unwrap();
        assert_eq!(detail.type_name, "User");
        assert_eq!(detail.field_name, "posts");
        assert!(detail.arg_count > 0); // has limit, offset args
    }

    #[test]
    fn field_detail_not_found() {
        let schema = test_schema();
        let coord: SchemaCoordinate = "Post.nonExistent".parse().unwrap();
        let result = schema.field_detail(&coord);
        assert!(result.is_err());
    }

    #[test]
    fn field_detail_deprecated() {
        let schema = test_schema();
        let coord: SchemaCoordinate = "Post.oldSlug".parse().unwrap();
        let detail = schema.field_detail(&coord).unwrap();
        assert!(detail.is_deprecated);
        assert!(detail.deprecation_reason.is_some());
    }

    #[test]
    fn field_detail_expands_input_types() {
        let schema = test_schema();
        let coord: SchemaCoordinate = "Mutation.createPost".parse().unwrap();
        let detail = schema.field_detail(&coord).unwrap();
        assert!(!detail.input_expansions.is_empty());
        assert!(
            detail
                .input_expansions
                .iter()
                .any(|t| t.name == "CreatePostInput")
        );
    }
}
