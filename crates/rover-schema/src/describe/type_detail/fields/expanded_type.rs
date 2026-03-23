use apollo_compiler::{
    Name,
    schema::{EnumType, ExtendedType, InputObjectType, InterfaceType, ObjectType, UnionType},
};

use super::{
    enum_value_info::EnumValueInfo, field_info::FieldInfo, input_field_info::InputFieldInfo,
};
use crate::{ParsedSchema, describe::deprecated::IsDeprecated};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ExpandedType {
    Object {
        name: Name,
        fields: Vec<FieldInfo>,
        implements: Vec<Name>,
        implementors: Vec<Name>,
    },
    Interface {
        name: Name,
        fields: Vec<FieldInfo>,
        implements: Vec<Name>,
        implementors: Vec<Name>,
    },
    Input {
        name: Name,
        fields: Vec<InputFieldInfo>,
    },
    Enum {
        name: Name,
        values: Vec<EnumValueInfo>,
    },
    Union {
        name: Name,
        members: Vec<Name>,
    },
}

impl ExpandedType {
    pub const fn name(&self) -> &Name {
        match self {
            ExpandedType::Object { name, .. }
            | ExpandedType::Interface { name, .. }
            | ExpandedType::Input { name, .. }
            | ExpandedType::Enum { name, .. }
            | ExpandedType::Union { name, .. } => name,
        }
    }
}

impl ParsedSchema {
    pub fn expand_single_type(
        &self,
        type_name: &str,
        include_deprecated: bool,
    ) -> Option<ExpandedType> {
        let (name, ty) = self.inner().types.get_key_value(type_name)?;
        match ty {
            ExtendedType::Object(obj) => {
                Some(self.expand_object(name.clone(), obj, include_deprecated))
            }
            ExtendedType::Interface(iface) => {
                Some(self.expand_interface(name.clone(), iface, include_deprecated))
            }
            ExtendedType::InputObject(inp) => Some(self.expand_input(name.clone(), inp)),
            ExtendedType::Enum(e) => Some(self.expand_enum(name.clone(), e, include_deprecated)),
            ExtendedType::Union(u) => Some(self.expand_union(name.clone(), u)),
            ExtendedType::Scalar(_) => None,
        }
    }

    fn expand_object(
        &self,
        name: Name,
        obj: &ObjectType,
        include_deprecated: bool,
    ) -> ExpandedType {
        let fields = obj
            .fields
            .iter()
            .filter(|(_, f)| include_deprecated || !f.is_deprecated())
            .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
            .collect();
        let implements = obj
            .implements_interfaces
            .iter()
            .map(|i| i.name.clone())
            .collect();
        ExpandedType::Object {
            name,
            fields,
            implements,
            implementors: Vec::new(),
        }
    }

    fn expand_interface(
        &self,
        name: Name,
        iface: &InterfaceType,
        include_deprecated: bool,
    ) -> ExpandedType {
        let fields = iface
            .fields
            .iter()
            .filter(|(_, f)| include_deprecated || !f.is_deprecated())
            .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
            .collect();
        let implements = iface
            .implements_interfaces
            .iter()
            .map(|i| i.name.clone())
            .collect();
        let implementors = self.find_implementors(&name);
        ExpandedType::Interface {
            name,
            fields,
            implements,
            implementors,
        }
    }

    fn expand_input(&self, name: Name, inp: &InputObjectType) -> ExpandedType {
        let fields = inp
            .fields
            .iter()
            .map(|(n, field)| InputFieldInfo::from_input_value_definition(n.clone(), field))
            .collect();
        ExpandedType::Input { name, fields }
    }

    fn expand_enum(&self, name: Name, e: &EnumType, include_deprecated: bool) -> ExpandedType {
        let values = e
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
        ExpandedType::Enum { name, values }
    }

    fn expand_union(&self, name: Name, u: &UnionType) -> ExpandedType {
        let members = u.members.iter().map(|m| m.name.clone()).collect();
        ExpandedType::Union { name, members }
    }
}
