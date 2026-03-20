use apollo_compiler::{Name, schema::ExtendedType};

use crate::{ParsedSchema, describe::deprecated::IsDeprecated};

use super::enum_value_info::EnumValueInfo;
use super::field_info::FieldInfo;
use super::type_kind::TypeKind;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ExpandedType {
    pub name: Name,
    pub kind: TypeKind,
    pub fields: Vec<FieldInfo>,
    pub enum_values: Vec<EnumValueInfo>,
    pub union_members: Vec<Name>,
    pub implements: Vec<Name>,
}

impl ParsedSchema {
    pub fn expand_single_type(&self, type_name: &str, include_deprecated: bool) -> Option<ExpandedType> {
        let (name, ty) = self.inner().types.get_key_value(type_name)?;
        match ty {
            ExtendedType::Object(obj) => {
                let fields: Vec<FieldInfo> = obj
                    .fields
                    .iter()
                    .filter(|(_, f)| include_deprecated || !f.is_deprecated())
                    .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
                    .collect();
                let implements: Vec<Name> = obj
                    .implements_interfaces
                    .iter()
                    .map(|i| i.name.clone())
                    .collect();
                Some(self.expand_fielded_type(name.clone(), TypeKind::Object, fields, implements))
            }
            ExtendedType::Interface(iface) => {
                let fields: Vec<FieldInfo> = iface
                    .fields
                    .iter()
                    .filter(|(_, f)| include_deprecated || !f.is_deprecated())
                    .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
                    .collect();
                Some(self.expand_fielded_type(name.clone(), TypeKind::Interface, fields, Vec::new()))
            }
            ExtendedType::InputObject(inp) => {
                let fields: Vec<FieldInfo> = inp
                    .fields
                    .iter()
                    .map(|(n, field)| FieldInfo::from_input_value_definition(n.clone(), field))
                    .collect();
                Some(ExpandedType {
                    name: name.clone(),
                    kind: TypeKind::Input,
                    fields,
                    enum_values: Vec::new(),
                    union_members: Vec::new(),
                    implements: Vec::new(),
                })
            }
            ExtendedType::Enum(e) => {
                let values: Vec<EnumValueInfo> = e
                    .values
                    .iter()
                    .filter(|(_, v)| include_deprecated || !v.is_deprecated())
                    .map(|(n, val)| EnumValueInfo {
                        name: n.clone(),
                        description: val.description.as_ref().map(|d| d.to_string()),
                        is_deprecated: val.is_deprecated(),
                        deprecation_reason: val.deprecation_reason(),
                    })
                    .collect();
                Some(ExpandedType {
                    name: name.clone(),
                    kind: TypeKind::Enum,
                    fields: Vec::new(),
                    enum_values: values,
                    union_members: Vec::new(),
                    implements: Vec::new(),
                })
            }
            ExtendedType::Union(u) => {
                let members: Vec<Name> = u.members.iter().map(|m| m.name.clone()).collect();
                Some(ExpandedType {
                    name: name.clone(),
                    kind: TypeKind::Union,
                    fields: Vec::new(),
                    enum_values: Vec::new(),
                    union_members: members,
                    implements: Vec::new(),
                })
            }
            ExtendedType::Scalar(_) => None,
        }
    }

    /// Shared logic for expanding Object and Interface types.
    pub(super) fn expand_fielded_type(
        &self,
        name: Name,
        kind: TypeKind,
        fields: Vec<FieldInfo>,
        implements: Vec<Name>,
    ) -> ExpandedType {
        let union_members = if kind == TypeKind::Interface {
            self.find_implementors(&name)
        } else {
            Vec::new()
        };
        ExpandedType { name, kind, fields, enum_values: Vec::new(), union_members, implements }
    }
}
