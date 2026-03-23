mod fields;
use apollo_compiler::{Name, schema::ExtendedType};
pub use fields::{
    ArgInfo, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail, FieldInfo,
    FieldSummary, FieldsDetail, TypeKind,
};

use super::deprecated::IsDeprecated;
use crate::{ParsedSchema, error::SchemaError, root_paths};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ObjectDetail {
    pub name: Name,
    pub description: Option<String>,
    pub implements: Vec<Name>,
    #[serde(flatten)]
    pub fields: ExtendedFieldsDetail,
    pub via: Vec<crate::root_paths::RootPath>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InterfaceDetail {
    pub name: Name,
    pub description: Option<String>,
    pub implements: Vec<Name>,
    #[serde(flatten)]
    pub fields: ExtendedFieldsDetail,
    pub implementors: Vec<Name>,
    pub via: Vec<crate::root_paths::RootPath>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InputDetail {
    pub name: Name,
    pub description: Option<String>,
    #[serde(flatten)]
    pub fields: FieldsDetail,
    pub via: Vec<crate::root_paths::RootPath>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumDetail {
    pub name: Name,
    pub description: Option<String>,
    pub values: Vec<EnumValueInfo>,
    pub value_count: usize,
    pub deprecated_count: usize,
    pub via: Vec<crate::root_paths::RootPath>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UnionDetail {
    pub name: Name,
    pub description: Option<String>,
    pub members: Vec<Name>,
    pub via: Vec<crate::root_paths::RootPath>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScalarDetail {
    pub name: Name,
    pub description: Option<String>,
    pub via: Vec<crate::root_paths::RootPath>,
}

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

        let via = self.find_root_paths(type_name);

        match ty {
            ExtendedType::Object(obj) => {
                let description = obj.description.as_ref().map(|d| d.to_string());
                let implements: Vec<Name> = obj
                    .implements_interfaces
                    .iter()
                    .map(|i| i.name.clone())
                    .collect();
                let all_fields: Vec<FieldInfo> = obj
                    .fields
                    .iter()
                    .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
                    .collect();
                let fields = self.extended_fields_detail(all_fields, include_deprecated, depth);
                Ok(TypeDetail::Object(ObjectDetail {
                    name: type_name.clone(),
                    description,
                    implements,
                    fields,
                    via,
                }))
            }
            ExtendedType::Interface(iface) => {
                let description = iface.description.as_ref().map(|d| d.to_string());
                let implements: Vec<Name> = iface
                    .implements_interfaces
                    .iter()
                    .map(|i| i.name.clone())
                    .collect();
                let all_fields: Vec<FieldInfo> = iface
                    .fields
                    .iter()
                    .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
                    .collect();
                let fields = self.extended_fields_detail(all_fields, include_deprecated, depth);
                let implementors = self.find_implementors(type_name);
                Ok(TypeDetail::Interface(InterfaceDetail {
                    name: type_name.clone(),
                    description,
                    implements,
                    fields,
                    implementors,
                    via,
                }))
            }
            ExtendedType::InputObject(inp) => {
                let description = inp.description.as_ref().map(|d| d.to_string());
                let fields: Vec<FieldInfo> = inp
                    .fields
                    .iter()
                    .map(|(n, field)| FieldInfo::from_input_value_definition(n.clone(), field))
                    .collect();
                let field_count = fields.len();
                Ok(TypeDetail::Input(InputDetail {
                    name: type_name.clone(),
                    description,
                    fields: FieldsDetail::new(fields, field_count),
                    via,
                }))
            }
            ExtendedType::Enum(e) => {
                let description = e.description.as_ref().map(|d| d.to_string());
                let all_values: Vec<EnumValueInfo> = e
                    .values
                    .iter()
                    .map(|(n, val)| EnumValueInfo {
                        name: n.clone(),
                        description: val.description.as_ref().map(|d| d.to_string()),
                        is_deprecated: val.is_deprecated(),
                        deprecation_reason: val.deprecation_reason(),
                    })
                    .collect();
                let value_count = all_values.len();
                let deprecated_count = all_values.iter().filter(|v| v.is_deprecated).count();
                let values = if include_deprecated {
                    all_values
                } else {
                    all_values
                        .into_iter()
                        .filter(|v| !v.is_deprecated)
                        .collect()
                };
                Ok(TypeDetail::Enum(EnumDetail {
                    name: type_name.clone(),
                    description,
                    values,
                    value_count,
                    deprecated_count,
                    via,
                }))
            }
            ExtendedType::Union(u) => {
                let description = u.description.as_ref().map(|d| d.to_string());
                let members: Vec<Name> = u.members.iter().map(|m| m.name.clone()).collect();
                Ok(TypeDetail::Union(UnionDetail {
                    name: type_name.clone(),
                    description,
                    members,
                    via,
                }))
            }
            ExtendedType::Scalar(s) => {
                let description = s.description.as_ref().map(|d| d.to_string());
                Ok(TypeDetail::Scalar(ScalarDetail {
                    name: type_name.clone(),
                    description,
                    via,
                }))
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
        assert_that!(inp.fields.fields().to_vec()).contains(&FieldInfo {
            name: Name::new("title").unwrap(),
            return_type: Name::new("String").unwrap(),
            description: Some("The post title".to_string()),
            is_deprecated: false,
            deprecation_reason: None,
            arg_count: 0,
        });
        assert_that!(inp.fields.fields().to_vec()).contains(&FieldInfo {
            name: Name::new("categoryId").unwrap(),
            return_type: Name::new("ID").unwrap(),
            description: Some("Category ID".to_string()),
            is_deprecated: false,
            deprecation_reason: None,
            arg_count: 0,
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
        assert_that!(obj.fields.expanded_types).matching_contains(|t| t.name == "User");
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
