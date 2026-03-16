mod fields;
pub use fields::{
    ArgInfo, EnumValueInfo, ExpandedType, ExtendedFieldsDetail, FieldDetail, FieldInfo,
    FieldSummary, FieldsDetail, TypeKind,
};

use apollo_compiler::{Name, schema::ExtendedType};

use crate::{ParsedSchema, error::SchemaError, root_paths};

use super::deprecated::IsDeprecated;

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

        let via = root_paths::find_root_paths(schema, type_name.as_str());

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
    use super::*;
    use crate::ParsedSchema;

    fn test_schema() -> ParsedSchema {
        let sdl = include_str!("../../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[test]
    fn type_detail_object() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("Post").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Object(obj) = detail else {
            panic!("expected Object variant")
        };
        assert_eq!(obj.name, "Post");
        assert!(obj.fields.field_count() > 0);
        assert!(!obj.implements.is_empty()); // Post implements Node & Timestamped
    }

    #[test]
    fn type_detail_enum() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("DigestFrequency").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Enum(e) = detail else {
            panic!("expected Enum variant")
        };
        assert_eq!(e.values.len(), 3); // DAILY, WEEKLY, NEVER
    }

    #[test]
    fn type_detail_interface() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("Timestamped").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Interface(iface) = detail else {
            panic!("expected Interface variant")
        };
        assert!(iface.implementors.iter().any(|n| n == "Post"));
        assert!(iface.implementors.iter().any(|n| n == "Comment"));
    }

    #[test]
    fn type_detail_input() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("CreatePostInput").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Input(inp) = detail else {
            panic!("expected Input variant")
        };
        assert!(inp.fields.fields().iter().any(|f| f.name == "title"));
    }

    #[test]
    fn type_detail_union() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("ContentItem").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Union(u) = detail else {
            panic!("expected Union variant")
        };
        assert!(u.members.iter().any(|n| n == "Post"));
        assert!(u.members.iter().any(|n| n == "Comment"));
    }

    #[test]
    fn type_detail_not_found() {
        let schema = test_schema();
        let result = schema.type_detail(&Name::new("NonExistent").unwrap(), true, 0);
        assert!(result.is_err());
    }

    #[test]
    fn type_detail_with_depth_expands_referenced_types() {
        let schema = test_schema();
        let detail = schema
            .type_detail(&Name::new("Post").unwrap(), true, 1)
            .unwrap();
        let TypeDetail::Object(obj) = detail else {
            panic!("expected Object variant")
        };
        assert!(!obj.fields.expanded_types.is_empty());
        assert!(obj.fields.expanded_types.iter().any(|t| t.name == "User"));
    }

    #[test]
    fn type_detail_deprecated_fields_filtered() {
        let schema = test_schema();
        let with_deprecated = schema
            .type_detail(&Name::new("User").unwrap(), true, 0)
            .unwrap();
        let without_deprecated = schema
            .type_detail(&Name::new("User").unwrap(), false, 0)
            .unwrap();
        let TypeDetail::Object(with_obj) = with_deprecated else {
            panic!()
        };
        let TypeDetail::Object(without_obj) = without_deprecated else {
            panic!()
        };
        assert!(with_obj.fields.fields().len() > without_obj.fields.fields().len());
        assert!(with_obj.fields.deprecated_count > 0);
    }

    #[test]
    fn type_detail_enum_deprecated_values() {
        let schema = test_schema();
        let with = schema
            .type_detail(&Name::new("SortOrder").unwrap(), true, 0)
            .unwrap();
        let TypeDetail::Enum(with_e) = with else {
            panic!()
        };
        assert_eq!(with_e.values.len(), 4);
        assert_eq!(with_e.deprecated_count, 1);
        let deprecated_val = with_e
            .values
            .iter()
            .find(|v| v.name == "RELEVANCE")
            .unwrap();
        assert!(deprecated_val.is_deprecated);
        assert_eq!(
            deprecated_val.deprecation_reason.as_deref(),
            Some("Use TOP instead")
        );

        let without = schema
            .type_detail(&Name::new("SortOrder").unwrap(), false, 0)
            .unwrap();
        let TypeDetail::Enum(without_e) = without else {
            panic!()
        };
        assert_eq!(without_e.values.len(), 3);
        assert!(!without_e.values.iter().any(|v| v.name == "RELEVANCE"));
    }
}
