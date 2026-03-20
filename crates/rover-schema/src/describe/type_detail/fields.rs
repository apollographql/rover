use apollo_compiler::{
    Name, Schema,
    coordinate::SchemaCoordinate,
    schema::{ExtendedType, FieldDefinition, InputValueDefinition},
};

use crate::{ParsedSchema, SchemaError, describe::deprecated::IsDeprecated, root_paths};

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumValueInfo {
    pub name: Name,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArgInfo {
    pub name: Name,
    pub arg_type: Name,
    pub description: Option<String>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeKind {
    Object,
    Input,
    Enum,
    Interface,
    Union,
    Scalar,
}

impl TypeKind {
    pub const fn label(&self) -> &'static str {
        match self {
            TypeKind::Object => "object",
            TypeKind::Input => "input",
            TypeKind::Enum => "enum",
            TypeKind::Interface => "interface",
            TypeKind::Union => "union",
            TypeKind::Scalar => "scalar",
        }
    }
}

impl std::fmt::Display for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExpandedType {
    pub name: Name,
    pub kind: TypeKind,
    pub fields: Vec<FieldInfo>,
    pub enum_values: Vec<EnumValueInfo>,
    pub union_members: Vec<Name>,
    pub implements: Vec<Name>,
}

#[derive(Debug, Clone, serde::Serialize, bon::Builder, derive_getters::Getters)]
pub struct FieldSummary {
    name: Name,
    return_type: String,
}

impl FieldSummary {
    pub(crate) fn new(schema: &Schema, root_name: &str) -> Vec<Self> {
        if let Some(ExtendedType::Object(obj)) = schema.types.get(root_name) {
            obj.fields
                .iter()
                .map(|(name, field)| FieldSummary {
                    name: name.clone(),
                    return_type: field.ty.to_string(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldInfo {
    pub name: Name,
    pub return_type: Name,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
    pub arg_count: usize,
}

impl FieldInfo {
    pub(super) fn from_field_definition(name: Name, field: &FieldDefinition) -> Self {
        Self {
            name,
            return_type: field.ty.inner_named_type().clone(),
            description: field.description.as_ref().map(|d| d.to_string()),
            is_deprecated: field.is_deprecated(),
            deprecation_reason: field.deprecation_reason(),
            arg_count: field.arguments.len(),
        }
    }

    pub(super) fn from_input_value_definition(name: Name, field: &InputValueDefinition) -> Self {
        Self {
            name,
            return_type: field.ty.inner_named_type().clone(),
            description: field.description.as_ref().map(|d| d.to_string()),
            is_deprecated: field.is_deprecated(),
            deprecation_reason: field.deprecation_reason(),
            arg_count: 0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldDetail {
    pub type_name: Name,
    pub field_name: Name,
    pub return_type: String,
    pub description: Option<String>,
    pub arg_count: usize,
    pub args: Vec<ArgInfo>,
    pub via: Vec<crate::root_paths::RootPath>,
    pub input_expansions: Vec<ExpandedType>,
    pub return_expansion: Option<ExpandedType>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldsDetail {
    fields: Vec<FieldInfo>,
    pub field_count: usize,
}

impl FieldsDetail {
    pub fn new(fields: Vec<FieldInfo>, field_count: usize) -> Self {
        Self {
            fields,
            field_count,
        }
    }

    pub fn fields(&self) -> &[FieldInfo] {
        &self.fields
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtendedFieldsDetail {
    #[serde(flatten)]
    fields: FieldsDetail,
    pub deprecated_count: usize,
    pub expanded_types: Vec<ExpandedType>,
}

impl ExtendedFieldsDetail {
    pub fn new(
        fields: FieldsDetail,
        deprecated_count: usize,
        expanded_types: Vec<ExpandedType>,
    ) -> Self {
        Self {
            fields,
            deprecated_count,
            expanded_types,
        }
    }

    pub fn fields(&self) -> &[FieldInfo] {
        self.fields.fields()
    }

    pub fn field_count(&self) -> usize {
        self.fields.field_count
    }
}

/// Process field list: count deprecated, filter if needed, expand referenced types.
impl ParsedSchema {
    /// Generate detailed info for a specific field.
    pub fn field_detail(&self, coord: &SchemaCoordinate) -> Result<FieldDetail, SchemaError> {
        let (type_name, field_name) = match coord {
            SchemaCoordinate::TypeAttribute(tac) => (tac.ty.clone(), tac.attribute.clone()),
            _ => {
                return Err(SchemaError::InvalidCoordinate(coord.clone()));
            }
        };

        let ty = self
            .inner()
            .types
            .get(type_name.as_str())
            .ok_or_else(|| SchemaError::TypeNotFound(type_name.clone()))?;

        let field = match ty {
            ExtendedType::Object(obj) => obj.fields.get(field_name.as_str()),
            ExtendedType::Interface(iface) => iface.fields.get(field_name.as_str()),
            _ => None,
        }
        .ok_or_else(|| SchemaError::FieldNotFound {
            type_name: type_name.clone(),
            field: field_name.clone(),
        })?;

        let return_type = field.ty.to_string();
        let description = field.description.as_ref().map(|d| d.to_string());
        let is_deprecated = field.is_deprecated();
        let deprecation_reason = field.deprecation_reason();

        let args: Vec<ArgInfo> = field
            .arguments
            .iter()
            .map(|arg| ArgInfo {
                name: arg.name.clone(),
                arg_type: arg.ty.inner_named_type().clone(),
                description: arg.description.as_ref().map(|d| d.to_string()),
                default_value: arg.default_value.as_ref().map(|v| v.to_string()),
            })
            .collect();

        let via = root_paths::find_root_paths(self.inner(), type_name.as_str());

        // Expand input types used as arguments
        let mut input_expansions = Vec::new();
        for arg in &args {
            if let Some(expanded) = self.expand_single_type(arg.arg_type.as_str(), true)
                && expanded.kind == TypeKind::Input
            {
                input_expansions.push(expanded);
            }
        }

        // Expand return type
        let return_expansion = self.expand_single_type(field.ty.inner_named_type().as_str(), true);

        Ok(FieldDetail {
            type_name,
            field_name,
            return_type,
            description,
            arg_count: args.len(),
            args,
            via,
            input_expansions,
            return_expansion,
            is_deprecated,
            deprecation_reason,
        })
    }

    pub(super) fn extended_fields_detail(
        &self,
        all_fields: Vec<FieldInfo>,
        include_deprecated: bool,
        depth: usize,
    ) -> ExtendedFieldsDetail {
        let deprecated_count = all_fields.iter().filter(|f| f.is_deprecated).count();
        let field_count = all_fields.len();
        let fields = if include_deprecated {
            all_fields
        } else {
            all_fields
                .into_iter()
                .filter(|f| !f.is_deprecated)
                .collect()
        };
        let expanded_types = if depth > 0 {
            self.expand_referenced_types(&fields, depth, include_deprecated)
        } else {
            Vec::new()
        };
        ExtendedFieldsDetail::new(
            FieldsDetail::new(fields, field_count),
            deprecated_count,
            expanded_types,
        )
    }

    fn expand_referenced_types(
        &self,
        fields: &[FieldInfo],
        depth: usize,
        include_deprecated: bool,
    ) -> Vec<ExpandedType> {
        if depth == 0 {
            return Vec::new();
        }

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for field in fields {
            if self
                .inner()
                .types
                .get(field.return_type.as_str())
                .map_or(true, |ty| ty.is_built_in())
            {
                continue;
            }
            if seen.contains(&field.return_type) {
                continue;
            }
            seen.insert(field.return_type.clone());
            if let Some(expanded) =
                self.expand_single_type(field.return_type.as_str(), include_deprecated)
            {
                result.push(expanded);
            }
        }

        result
    }

    pub fn expand_single_type(
        &self,
        type_name: &str,
        include_deprecated: bool,
    ) -> Option<ExpandedType> {
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
                Some(self.expand_fielded_type(
                    name.clone(),
                    TypeKind::Interface,
                    fields,
                    Vec::new(),
                ))
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
    fn expand_fielded_type(
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
        ExpandedType {
            name,
            kind,
            fields,
            enum_values: Vec::new(),
            union_members,
            implements,
        }
    }
}
