mod enum_detail;
mod fields;
mod input_detail;
mod interface_detail;
mod object_detail;
mod scalar_detail;
mod union_detail;

use apollo_compiler::{Name, schema::ExtendedType};
pub use enum_detail::EnumDetail;
pub use fields::{
    ArgInfo, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail, FieldInfo,
    FieldSummary, FieldsDetail, InputFieldInfo,
};
pub use input_detail::InputDetail;
pub use interface_detail::InterfaceDetail;
pub use object_detail::ObjectDetail;
pub use scalar_detail::ScalarDetail;
pub use union_detail::UnionDetail;

use crate::{ParsedSchema, error::SchemaError};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TypeDetail {
    Object(ObjectDetail),
    Interface(InterfaceDetail),
    Input(InputDetail),
    Enum(EnumDetail),
    Union(UnionDetail),
    Scalar(ScalarDetail),
}

impl ParsedSchema {
    pub fn type_detail(
        &self,
        type_name: &Name,
        include_deprecated: bool,
        depth: usize,
    ) -> Result<TypeDetail, SchemaError> {
        let schema = self.inner();
        let ty = schema
            .types
            .get(type_name)
            .ok_or_else(|| SchemaError::TypeNotFound(type_name.clone()))?;

        match ty {
            ExtendedType::Object(obj) => Ok(TypeDetail::Object(self.build_object_detail(
                type_name,
                obj,
                include_deprecated,
                depth,
            ))),
            ExtendedType::Interface(iface) => Ok(TypeDetail::Interface(
                self.build_interface_detail(type_name, iface, include_deprecated, depth),
            )),
            ExtendedType::InputObject(inp) => {
                Ok(TypeDetail::Input(self.build_input_detail(type_name, inp)))
            }
            ExtendedType::Enum(e) => Ok(TypeDetail::Enum(self.build_enum_detail(
                type_name,
                e,
                include_deprecated,
            ))),
            ExtendedType::Union(u) => Ok(TypeDetail::Union(self.build_union_detail(type_name, u))),
            ExtendedType::Scalar(s) => {
                Ok(TypeDetail::Scalar(self.build_scalar_detail(type_name, s)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;
    use crate::ParsedSchema;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("../../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[rstest]
    fn type_detail_object(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Post").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Object(obj) = detail else {
            panic!("expected Object variant")
        };
        assert_that!(obj.name).is_equal_to(Name::new("Post").unwrap());
        assert_that!(obj.fields.field_count()).is_equal_to(14);
        assert_that!(obj.implements).is_equal_to(vec![
            Name::new("Node").unwrap(),
            Name::new("Timestamped").unwrap(),
        ]);
    }

    #[rstest]
    fn type_detail_enum(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("DigestFrequency").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Enum(e) = detail else {
            panic!("expected Enum variant")
        };
        assert_that!(e.values).is_equal_to(vec![
            EnumValueInfo {
                name: Name::new("DAILY").unwrap(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
            },
            EnumValueInfo {
                name: Name::new("WEEKLY").unwrap(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
            },
            EnumValueInfo {
                name: Name::new("NEVER").unwrap(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
            },
        ]);
    }

    #[rstest]
    fn type_detail_interface(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Timestamped").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Interface(iface) = detail else {
            panic!("expected Interface variant")
        };
        assert_that!(iface.implementors).contains(&Name::new("Post").unwrap());
        assert_that!(iface.implementors).contains(&Name::new("Comment").unwrap());
    }

    #[rstest]
    fn type_detail_input(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("CreatePostInput").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Input(inp) = detail else {
            panic!("expected Input variant")
        };
        assert_that!(inp.fields).contains(&InputFieldInfo {
            name: Name::new("title").unwrap(),
            field_type: Name::new("String").unwrap(),
            description: Some("The post title".to_string()),
            is_deprecated: false,
            deprecation_reason: None,
        });
        assert_that!(inp.fields).contains(&InputFieldInfo {
            name: Name::new("categoryId").unwrap(),
            field_type: Name::new("ID").unwrap(),
            description: Some("Category ID".to_string()),
            is_deprecated: false,
            deprecation_reason: None,
        });
    }

    #[rstest]
    fn type_detail_union(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("ContentItem").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Union(u) = detail else {
            panic!("expected Union variant")
        };
        assert_that!(u.members).contains(&Name::new("Post").unwrap());
        assert_that!(u.members).contains(&Name::new("Comment").unwrap());
    }

    #[rstest]
    fn type_detail_not_found(schema: ParsedSchema) {
        let result = schema.type_detail(&Name::new("NonExistent").unwrap(), true, 0);
        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, SchemaError::TypeNotFound(n) if n.as_str() == "NonExistent"));
    }

    #[rstest]
    fn type_detail_with_depth_expands_referenced_types(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Post").unwrap(), true, 1)
            .unwrap();
        let TypeDetail::Object(obj) = detail else {
            panic!("expected Object variant")
        };
        assert_that!(obj.fields.expanded_types).is_not_empty();
        assert_that!(obj.fields.expanded_types).matching_contains(|t| t.name() == "User");
    }

    #[rstest]
    fn type_detail_deprecated_fields_filtered(schema: ParsedSchema) {
        let with_obj = match schema
            .type_detail(&Name::new("User").unwrap(), true, 0)
            .unwrap()
        {
            TypeDetail::Object(o) => o,
            _ => panic!("expected Object variant"),
        };
        let without_obj = match schema
            .type_detail(&Name::new("User").unwrap(), false, 0)
            .unwrap()
        {
            TypeDetail::Object(o) => o,
            _ => panic!("expected Object variant"),
        };
        let legacy_id = FieldInfo {
            name: Name::new("legacyId").unwrap(),
            return_type: Name::new("String").unwrap(),
            description: None,
            is_deprecated: true,
            deprecation_reason: Some("Use id instead".to_string()),
            arg_count: 0,
        };
        assert_that!(with_obj.fields.fields().to_vec()).contains(&legacy_id);
        assert_that!(without_obj.fields.fields().to_vec()).does_not_contain(&legacy_id);
        assert_that!(with_obj.fields.deprecated_count).is_equal_to(1);
    }

    #[rstest]
    fn type_detail_enum_deprecated_values(schema: ParsedSchema) {
        let relevance = EnumValueInfo {
            name: Name::new("RELEVANCE").unwrap(),
            description: None,
            is_deprecated: true,
            deprecation_reason: Some("Use TOP instead".to_string()),
        };

        let with_e = match schema
            .type_detail(&Name::new("SortOrder").unwrap(), true, 0)
            .unwrap()
        {
            TypeDetail::Enum(e) => e,
            _ => panic!("expected Enum variant"),
        };
        assert_that!(with_e.values).has_length(4);
        assert_that!(with_e.deprecated_count).is_equal_to(1);
        assert_that!(with_e.values).contains(&relevance);

        let without_e = match schema
            .type_detail(&Name::new("SortOrder").unwrap(), false, 0)
            .unwrap()
        {
            TypeDetail::Enum(e) => e,
            _ => panic!("expected Enum variant"),
        };
        assert_that!(without_e.values).has_length(3);
        assert_that!(without_e.values).does_not_contain(&relevance);
    }
}
