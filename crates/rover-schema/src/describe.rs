use apollo_compiler::{
    Schema,
    schema::{ExtendedType, FieldDefinition, InputValueDefinition},
};

use crate::{
    coordinate::SchemaCoordinate, error::SchemaError, format::ARROW, root_paths,
    util::unwrap_type_name,
};

/// Result of describing a schema at different levels of detail.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum DescribeResult {
    Overview(SchemaOverview),
    TypeDetail(TypeDetail),
    FieldDetail(FieldDetail),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaOverview {
    pub graph_ref: String,
    pub total_types: usize,
    pub total_fields: usize,
    pub total_deprecated: usize,
    pub query_fields: Vec<FieldSummary>,
    pub mutation_fields: Vec<FieldSummary>,
    pub objects: Vec<String>,
    pub inputs: Vec<String>,
    pub enums: Vec<String>,
    pub interfaces: Vec<String>,
    pub unions: Vec<String>,
    pub scalars: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeDetail {
    pub name: String,
    pub kind: TypeKind,
    pub description: Option<String>,
    pub field_count: usize,
    pub deprecated_count: usize,
    pub implements: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub enum_values: Vec<EnumValueInfo>,
    pub union_members: Vec<String>,
    pub via: Vec<String>,
    pub expanded_types: Vec<ExpandedType>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldDetail {
    pub type_name: String,
    pub field_name: String,
    pub return_type: String,
    pub description: Option<String>,
    pub arg_count: usize,
    pub args: Vec<ArgInfo>,
    pub via: Vec<String>,
    pub input_expansions: Vec<ExpandedType>,
    pub return_expansion: Option<ExpandedType>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldSummary {
    pub name: String,
    pub return_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldInfo {
    pub name: String,
    pub return_type: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
    pub arg_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumValueInfo {
    pub name: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArgInfo {
    pub name: String,
    pub arg_type: String,
    pub description: Option<String>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExpandedType {
    pub name: String,
    pub kind: TypeKind,
    pub fields: Vec<FieldInfo>,
    pub enum_values: Vec<EnumValueInfo>,
    pub union_members: Vec<String>,
    pub implements: Vec<String>,
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

/// Generate a schema overview.
pub fn overview(schema: &Schema, graph_ref: &str) -> SchemaOverview {
    let mut total_fields = 0usize;
    let mut total_deprecated = 0usize;
    let mut objects = Vec::new();
    let mut inputs = Vec::new();
    let mut enums = Vec::new();
    let mut interfaces = Vec::new();
    let mut unions = Vec::new();
    let mut scalars = Vec::new();

    let builtin_scalars = ["String", "Int", "Float", "Boolean", "ID"];

    for (name, ty) in &schema.types {
        let name_str = name.to_string();
        // Skip built-in types
        if name_str.starts_with("__") || builtin_scalars.contains(&name_str.as_str()) {
            continue;
        }
        match ty {
            ExtendedType::Object(obj) => {
                let fc = obj.fields.len();
                let dc = count_deprecated_fields_obj(obj);
                total_fields += fc;
                total_deprecated += dc;
                if name_str != "Query" && name_str != "Mutation" && name_str != "Subscription" {
                    objects.push(name_str);
                }
            }
            ExtendedType::InputObject(inp) => {
                total_fields += inp.fields.len();
                inputs.push(name_str);
            }
            ExtendedType::Enum(e) => {
                let dc = e
                    .values
                    .values()
                    .filter(|v| is_deprecated_directive(&v.directives))
                    .count();
                total_deprecated += dc;
                enums.push(name_str);
            }
            ExtendedType::Interface(iface) => {
                let fc = iface.fields.len();
                let dc = count_deprecated_fields_iface(iface);
                total_fields += fc;
                total_deprecated += dc;
                interfaces.push(name_str);
            }
            ExtendedType::Union(_) => {
                unions.push(name_str);
            }
            ExtendedType::Scalar(_) => {
                scalars.push(name_str);
            }
        }
    }

    objects.sort();
    inputs.sort();
    enums.sort();
    interfaces.sort();
    unions.sort();
    scalars.sort();

    let user_types = objects.len()
        + inputs.len()
        + enums.len()
        + interfaces.len()
        + unions.len()
        + scalars.len()
        + count_root_types(schema);

    let query_fields = get_root_fields(schema, "Query");
    let mutation_fields = get_root_fields(schema, "Mutation");

    // Add Query/Mutation fields to total
    total_fields += query_fields.len();
    total_fields += mutation_fields.len();

    SchemaOverview {
        graph_ref: graph_ref.to_string(),
        total_types: user_types,
        total_fields,
        total_deprecated,
        query_fields,
        mutation_fields,
        objects,
        inputs,
        enums,
        interfaces,
        unions,
        scalars,
    }
}

/// Generate detailed info for a type.
pub fn type_detail(
    schema: &Schema,
    type_name: &str,
    include_deprecated: bool,
    depth: usize,
) -> Result<TypeDetail, SchemaError> {
    let ty = schema
        .types
        .get(type_name)
        .ok_or_else(|| SchemaError::TypeNotFound(type_name.to_string()))?;

    let via_paths = root_paths::find_root_paths(schema, type_name);
    let via: Vec<String> = via_paths.iter().map(|p| p.format_via()).collect();

    match ty {
        ExtendedType::Object(obj) => {
            let description = obj.description.as_ref().map(|d| d.to_string());
            let implements: Vec<String> = obj
                .implements_interfaces
                .iter()
                .map(|i| i.to_string())
                .collect();
            let all_fields: Vec<FieldInfo> = obj
                .fields
                .iter()
                .map(|(name, field)| field_to_info(name.as_str(), field))
                .collect();
            let (fields, field_count, deprecated_count, expanded_types) =
                process_fields(schema, all_fields, include_deprecated, depth);
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Object,
                description,
                field_count,
                deprecated_count,
                implements,
                fields,
                enum_values: Vec::new(),
                union_members: Vec::new(),
                via,
                expanded_types,
            })
        }
        ExtendedType::Interface(iface) => {
            let description = iface.description.as_ref().map(|d| d.to_string());
            let implements: Vec<String> = iface
                .implements_interfaces
                .iter()
                .map(|i| i.to_string())
                .collect();
            let all_fields: Vec<FieldInfo> = iface
                .fields
                .iter()
                .map(|(name, field)| field_to_info(name.as_str(), field))
                .collect();
            let (fields, field_count, deprecated_count, expanded_types) =
                process_fields(schema, all_fields, include_deprecated, depth);
            let implementors = find_implementors(schema, type_name);
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Interface,
                description,
                field_count,
                deprecated_count,
                implements,
                fields,
                enum_values: Vec::new(),
                union_members: implementors,
                via,
                expanded_types,
            })
        }
        ExtendedType::InputObject(inp) => {
            let description = inp.description.as_ref().map(|d| d.to_string());
            let fields: Vec<FieldInfo> = inp
                .fields
                .iter()
                .map(|(name, field)| input_field_to_info(name.as_str(), field))
                .collect();
            let field_count = fields.len();
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Input,
                description,
                field_count,
                deprecated_count: 0,
                implements: Vec::new(),
                fields,
                enum_values: Vec::new(),
                union_members: Vec::new(),
                via,
                expanded_types: Vec::new(),
            })
        }
        ExtendedType::Enum(e) => {
            let description = e.description.as_ref().map(|d| d.to_string());
            let all_values: Vec<EnumValueInfo> = e
                .values
                .iter()
                .map(|(name, val)| EnumValueInfo {
                    name: name.to_string(),
                    description: val.description.as_ref().map(|d| d.to_string()),
                    is_deprecated: is_deprecated_directive(&val.directives),
                    deprecation_reason: get_deprecation_reason(&val.directives),
                })
                .collect();
            let deprecated_count = all_values.iter().filter(|v| v.is_deprecated).count();
            let values = if include_deprecated {
                all_values
            } else {
                all_values
                    .into_iter()
                    .filter(|v| !v.is_deprecated)
                    .collect()
            };
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Enum,
                description,
                field_count: values.len(),
                deprecated_count,
                implements: Vec::new(),
                fields: Vec::new(),
                enum_values: values,
                union_members: Vec::new(),
                via,
                expanded_types: Vec::new(),
            })
        }
        ExtendedType::Union(u) => {
            let description = u.description.as_ref().map(|d| d.to_string());
            let members: Vec<String> = u.members.iter().map(|m| m.to_string()).collect();
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Union,
                description,
                field_count: 0,
                deprecated_count: 0,
                implements: Vec::new(),
                fields: Vec::new(),
                enum_values: Vec::new(),
                union_members: members,
                via,
                expanded_types: Vec::new(),
            })
        }
        ExtendedType::Scalar(s) => {
            let description = s.description.as_ref().map(|d| d.to_string());
            Ok(TypeDetail {
                name: type_name.to_string(),
                kind: TypeKind::Scalar,
                description,
                field_count: 0,
                deprecated_count: 0,
                implements: Vec::new(),
                fields: Vec::new(),
                enum_values: Vec::new(),
                union_members: Vec::new(),
                via,
                expanded_types: Vec::new(),
            })
        }
    }
}

/// Generate detailed info for a specific field.
pub fn field_detail(schema: &Schema, coord: &SchemaCoordinate) -> Result<FieldDetail, SchemaError> {
    let (type_name, field_name) = match coord {
        SchemaCoordinate::Field {
            type_name,
            field_name,
        } => (type_name.as_str(), field_name.as_str()),
        _ => {
            return Err(SchemaError::InvalidCoordinate(
                "field_detail requires a Type.field coordinate".into(),
            ));
        }
    };

    let ty = schema
        .types
        .get(type_name)
        .ok_or_else(|| SchemaError::TypeNotFound(type_name.to_string()))?;

    let field = match ty {
        ExtendedType::Object(obj) => obj.fields.get(field_name),
        ExtendedType::Interface(iface) => iface.fields.get(field_name),
        _ => None,
    }
    .ok_or_else(|| SchemaError::FieldNotFound {
        type_name: type_name.to_string(),
        field: field_name.to_string(),
    })?;

    let return_type = field.ty.to_string();
    let description = field.description.as_ref().map(|d| d.to_string());
    let is_deprecated = is_deprecated_directive(&field.directives);
    let deprecation_reason = get_deprecation_reason(&field.directives);

    let args: Vec<ArgInfo> = field
        .arguments
        .iter()
        .map(|arg| ArgInfo {
            name: arg.name.to_string(),
            arg_type: arg.ty.to_string(),
            description: arg.description.as_ref().map(|d| d.to_string()),
            default_value: arg.default_value.as_ref().map(|v| v.to_string()),
        })
        .collect();

    let via_paths = root_paths::find_root_paths(schema, type_name);
    let via: Vec<String> = via_paths
        .iter()
        .map(|p| format!("{} {ARROW} {}.{}", p.format_via(), type_name, field_name))
        .collect();

    // Expand input types used as arguments
    let mut input_expansions = Vec::new();
    for arg in &args {
        let arg_type_name = unwrap_type_name(&arg.arg_type);
        if let Some(expanded) = expand_single_type(schema, &arg_type_name, true)
            && expanded.kind == TypeKind::Input
        {
            input_expansions.push(expanded);
        }
    }

    // Expand return type
    let return_type_name = unwrap_type_name(&return_type);
    let return_expansion = expand_single_type(schema, &return_type_name, true);

    Ok(FieldDetail {
        type_name: type_name.to_string(),
        field_name: field_name.to_string(),
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

// Helper functions

/// Process field list: count deprecated, filter if needed, expand referenced types.
fn process_fields(
    schema: &Schema,
    all_fields: Vec<FieldInfo>,
    include_deprecated: bool,
    depth: usize,
) -> (Vec<FieldInfo>, usize, usize, Vec<ExpandedType>) {
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
        expand_referenced_types(schema, &fields, depth, include_deprecated)
    } else {
        Vec::new()
    };
    (fields, field_count, deprecated_count, expanded_types)
}

fn field_to_info(name: &str, field: &FieldDefinition) -> FieldInfo {
    FieldInfo {
        name: name.to_string(),
        return_type: field.ty.to_string(),
        description: field.description.as_ref().map(|d| d.to_string()),
        is_deprecated: is_deprecated_directive(&field.directives),
        deprecation_reason: get_deprecation_reason(&field.directives),
        arg_count: field.arguments.len(),
    }
}

fn input_field_to_info(name: &str, field: &InputValueDefinition) -> FieldInfo {
    FieldInfo {
        name: name.to_string(),
        return_type: field.ty.to_string(),
        description: field.description.as_ref().map(|d| d.to_string()),
        is_deprecated: is_deprecated_directive(&field.directives),
        deprecation_reason: get_deprecation_reason(&field.directives),
        arg_count: 0,
    }
}

fn is_deprecated_directive(directives: &apollo_compiler::ast::DirectiveList) -> bool {
    directives.get("deprecated").is_some()
}

fn get_deprecation_reason(directives: &apollo_compiler::ast::DirectiveList) -> Option<String> {
    directives.get("deprecated").and_then(|d| {
        d.arguments
            .iter()
            .find(|arg| arg.name == "reason")
            .and_then(|arg| {
                if let apollo_compiler::ast::Value::String(s) = &*arg.value {
                    Some(s.to_string())
                } else {
                    None
                }
            })
    })
}

fn get_root_fields(schema: &Schema, root_name: &str) -> Vec<FieldSummary> {
    if let Some(ExtendedType::Object(obj)) = schema.types.get(root_name) {
        obj.fields
            .iter()
            .map(|(name, field)| FieldSummary {
                name: name.to_string(),
                return_type: field.ty.to_string(),
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn count_root_types(schema: &Schema) -> usize {
    ["Query", "Mutation", "Subscription"]
        .into_iter()
        .filter(|name| schema.types.contains_key(*name))
        .count()
}

fn count_deprecated_fields_obj(obj: &apollo_compiler::schema::ObjectType) -> usize {
    obj.fields
        .values()
        .filter(|f| is_deprecated_directive(&f.directives))
        .count()
}

fn count_deprecated_fields_iface(iface: &apollo_compiler::schema::InterfaceType) -> usize {
    iface
        .fields
        .values()
        .filter(|f| is_deprecated_directive(&f.directives))
        .count()
}

fn find_implementors(schema: &Schema, interface_name: &str) -> Vec<String> {
    let mut implementors = Vec::new();
    for (name, ty) in &schema.types {
        if let ExtendedType::Object(obj) = ty
            && obj
                .implements_interfaces
                .iter()
                .any(|i| i.as_str() == interface_name)
        {
            implementors.push(name.to_string());
        }
    }
    implementors.sort();
    implementors
}

pub fn expand_single_type(
    schema: &Schema,
    type_name: &str,
    include_deprecated: bool,
) -> Option<ExpandedType> {
    let ty = schema.types.get(type_name)?;
    match ty {
        ExtendedType::Object(obj) => {
            let fields: Vec<FieldInfo> = obj
                .fields
                .iter()
                .filter(|(_, f)| include_deprecated || !is_deprecated_directive(&f.directives))
                .map(|(name, field)| field_to_info(name.as_str(), field))
                .collect();
            let implements: Vec<String> = obj
                .implements_interfaces
                .iter()
                .map(|i| i.to_string())
                .collect();
            Some(expand_fielded_type(
                schema,
                type_name,
                TypeKind::Object,
                fields,
                implements,
            ))
        }
        ExtendedType::Interface(iface) => {
            let fields: Vec<FieldInfo> = iface
                .fields
                .iter()
                .filter(|(_, f)| include_deprecated || !is_deprecated_directive(&f.directives))
                .map(|(name, field)| field_to_info(name.as_str(), field))
                .collect();
            Some(expand_fielded_type(
                schema,
                type_name,
                TypeKind::Interface,
                fields,
                Vec::new(),
            ))
        }
        ExtendedType::InputObject(inp) => {
            let fields: Vec<FieldInfo> = inp
                .fields
                .iter()
                .map(|(name, field)| input_field_to_info(name.as_str(), field))
                .collect();
            Some(ExpandedType {
                name: type_name.to_string(),
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
                .filter(|(_, v)| include_deprecated || !is_deprecated_directive(&v.directives))
                .map(|(name, val)| EnumValueInfo {
                    name: name.to_string(),
                    description: val.description.as_ref().map(|d| d.to_string()),
                    is_deprecated: is_deprecated_directive(&val.directives),
                    deprecation_reason: get_deprecation_reason(&val.directives),
                })
                .collect();
            Some(ExpandedType {
                name: type_name.to_string(),
                kind: TypeKind::Enum,
                fields: Vec::new(),
                enum_values: values,
                union_members: Vec::new(),
                implements: Vec::new(),
            })
        }
        ExtendedType::Union(u) => {
            let members: Vec<String> = u.members.iter().map(|m| m.to_string()).collect();
            Some(ExpandedType {
                name: type_name.to_string(),
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
    schema: &Schema,
    type_name: &str,
    kind: TypeKind,
    fields: Vec<FieldInfo>,
    implements: Vec<String>,
) -> ExpandedType {
    let union_members = if kind == TypeKind::Interface {
        find_implementors(schema, type_name)
    } else {
        Vec::new()
    };
    ExpandedType {
        name: type_name.to_string(),
        kind,
        fields,
        enum_values: Vec::new(),
        union_members,
        implements,
    }
}

fn expand_referenced_types(
    schema: &Schema,
    fields: &[FieldInfo],
    depth: usize,
    include_deprecated: bool,
) -> Vec<ExpandedType> {
    if depth == 0 {
        return Vec::new();
    }

    let builtin_scalars = ["String", "Int", "Float", "Boolean", "ID"];
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for field in fields {
        let type_name = unwrap_type_name(&field.return_type);
        if builtin_scalars.contains(&type_name.as_str()) || type_name.starts_with("__") {
            continue;
        }
        if seen.contains(&type_name) {
            continue;
        }
        seen.insert(type_name.clone());
        if let Some(expanded) = expand_single_type(schema, &type_name, include_deprecated) {
            result.push(expanded);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> Schema {
        let sdl = include_str!("test_fixtures/test_schema.graphql");
        match Schema::parse(sdl, "test.graphql") {
            Ok(s) => s,
            Err(e) => e.partial,
        }
    }

    #[test]
    fn overview_type_counts() {
        let schema = test_schema();
        let ov = overview(&schema, "test@current");
        assert!(ov.total_types > 0);
        assert!(!ov.query_fields.is_empty());
        assert!(!ov.mutation_fields.is_empty());
        assert!(!ov.objects.is_empty());
        assert!(!ov.enums.is_empty());
        assert!(!ov.interfaces.is_empty());
        assert!(!ov.inputs.is_empty());
    }

    #[test]
    fn overview_excludes_builtins() {
        let schema = test_schema();
        let ov = overview(&schema, "test@current");
        assert!(!ov.objects.contains(&"__Schema".to_string()));
        assert!(!ov.scalars.contains(&"String".to_string()));
    }

    #[test]
    fn overview_excludes_root_types_from_objects() {
        let schema = test_schema();
        let ov = overview(&schema, "test@current");
        assert!(!ov.objects.contains(&"Query".to_string()));
        assert!(!ov.objects.contains(&"Mutation".to_string()));
    }

    #[test]
    fn type_detail_object() {
        let schema = test_schema();
        let detail = type_detail(&schema, "Post", true, 0).unwrap();
        assert_eq!(detail.name, "Post");
        assert_eq!(detail.kind, TypeKind::Object);
        assert!(detail.field_count > 0);
        assert!(!detail.implements.is_empty()); // Post implements Node & Timestamped
    }

    #[test]
    fn type_detail_enum() {
        let schema = test_schema();
        let detail = type_detail(&schema, "DigestFrequency", true, 0).unwrap();
        assert_eq!(detail.kind, TypeKind::Enum);
        assert_eq!(detail.enum_values.len(), 3); // DAILY, WEEKLY, NEVER
    }

    #[test]
    fn type_detail_interface() {
        let schema = test_schema();
        let detail = type_detail(&schema, "Timestamped", true, 0).unwrap();
        assert_eq!(detail.kind, TypeKind::Interface);
        assert!(detail.union_members.contains(&"Post".to_string()));
        assert!(detail.union_members.contains(&"Comment".to_string()));
    }

    #[test]
    fn type_detail_input() {
        let schema = test_schema();
        let detail = type_detail(&schema, "CreatePostInput", true, 0).unwrap();
        assert_eq!(detail.kind, TypeKind::Input);
        assert!(detail.fields.iter().any(|f| f.name == "title"));
    }

    #[test]
    fn type_detail_union() {
        let schema = test_schema();
        let detail = type_detail(&schema, "ContentItem", true, 0).unwrap();
        assert_eq!(detail.kind, TypeKind::Union);
        assert!(detail.union_members.contains(&"Post".to_string()));
        assert!(detail.union_members.contains(&"Comment".to_string()));
    }

    #[test]
    fn type_detail_not_found() {
        let schema = test_schema();
        let result = type_detail(&schema, "NonExistent", true, 0);
        assert!(result.is_err());
    }

    #[test]
    fn type_detail_with_depth_expands_referenced_types() {
        let schema = test_schema();
        let detail = type_detail(&schema, "Post", true, 1).unwrap();
        assert!(!detail.expanded_types.is_empty());
        assert!(detail.expanded_types.iter().any(|t| t.name == "User"));
    }

    #[test]
    fn type_detail_deprecated_fields_filtered() {
        let schema = test_schema();
        let with_deprecated = type_detail(&schema, "User", true, 0).unwrap();
        let without_deprecated = type_detail(&schema, "User", false, 0).unwrap();
        assert!(with_deprecated.fields.len() > without_deprecated.fields.len());
        assert!(with_deprecated.deprecated_count > 0);
    }

    #[test]
    fn field_detail_with_args() {
        let schema = test_schema();
        let coord = SchemaCoordinate::Field {
            type_name: "User".into(),
            field_name: "posts".into(),
        };
        let detail = field_detail(&schema, &coord).unwrap();
        assert_eq!(detail.type_name, "User");
        assert_eq!(detail.field_name, "posts");
        assert!(detail.arg_count > 0); // has limit, offset args
    }

    #[test]
    fn field_detail_not_found() {
        let schema = test_schema();
        let coord = SchemaCoordinate::Field {
            type_name: "Post".into(),
            field_name: "nonExistent".into(),
        };
        let result = field_detail(&schema, &coord);
        assert!(result.is_err());
    }

    #[test]
    fn field_detail_deprecated() {
        let schema = test_schema();
        let coord = SchemaCoordinate::Field {
            type_name: "Post".into(),
            field_name: "oldSlug".into(),
        };
        let detail = field_detail(&schema, &coord).unwrap();
        assert!(detail.is_deprecated);
        assert!(detail.deprecation_reason.is_some());
    }

    #[test]
    fn type_detail_enum_deprecated_values() {
        let schema = test_schema();
        let with = type_detail(&schema, "SortOrder", true, 0).unwrap();
        assert_eq!(with.enum_values.len(), 4);
        assert_eq!(with.deprecated_count, 1);
        let deprecated_val = with
            .enum_values
            .iter()
            .find(|v| v.name == "RELEVANCE")
            .unwrap();
        assert!(deprecated_val.is_deprecated);
        assert_eq!(
            deprecated_val.deprecation_reason.as_deref(),
            Some("Use TOP instead")
        );

        let without = type_detail(&schema, "SortOrder", false, 0).unwrap();
        assert_eq!(without.enum_values.len(), 3);
        assert!(!without.enum_values.iter().any(|v| v.name == "RELEVANCE"));
    }

    #[test]
    fn field_detail_expands_input_types() {
        let schema = test_schema();
        let coord = SchemaCoordinate::Field {
            type_name: "Mutation".into(),
            field_name: "createPost".into(),
        };
        let detail = field_detail(&schema, &coord).unwrap();
        assert!(!detail.input_expansions.is_empty());
        assert!(
            detail
                .input_expansions
                .iter()
                .any(|t| t.name == "CreatePostInput")
        );
    }
}
