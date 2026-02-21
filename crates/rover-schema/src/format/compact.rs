use crate::describe::{
    DescribeResult, ExpandedType, FieldDetail, SchemaOverview, TypeDetail, TypeKind,
};
#[cfg(feature = "search")]
use crate::search::SearchResult;

/// Format a DescribeResult in compact (token-efficient) notation.
pub fn format_describe_compact(result: &DescribeResult) -> String {
    match result {
        DescribeResult::Overview(overview) => format_overview_compact(overview),
        DescribeResult::TypeDetail(detail) => format_type_detail_compact(detail),
        DescribeResult::FieldDetail(detail) => format_field_detail_compact(detail),
    }
}

fn format_overview_compact(ov: &SchemaOverview) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "S:{}:{}t,{}f",
        ov.graph_ref, ov.total_types, ov.total_fields
    ));
    if ov.total_deprecated > 0 {
        out.push_str(&format!(",{}d", ov.total_deprecated));
    }
    out.push('\n');

    if !ov.query_fields.is_empty() {
        let fields: Vec<String> = ov
            .query_fields
            .iter()
            .map(|f| format!("{}:{}", f.name, abbreviate_type(&f.return_type)))
            .collect();
        out.push_str(&format!("Q:{}\n", fields.join(",")));
    }
    if !ov.mutation_fields.is_empty() {
        let fields: Vec<String> = ov
            .mutation_fields
            .iter()
            .map(|f| format!("{}:{}", f.name, abbreviate_type(&f.return_type)))
            .collect();
        out.push_str(&format!("M:{}\n", fields.join(",")));
    }

    out.trim_end().to_string()
}

fn format_type_detail_compact(detail: &TypeDetail) -> String {
    let mut out = String::new();

    let prefix = kind_prefix(detail.kind);
    out.push_str(prefix);
    out.push_str(&detail.name);

    // For interfaces, show implementing types
    if detail.kind == TypeKind::Interface && !detail.union_members.is_empty() {
        out.push_str(&format!("<{}>", detail.union_members.join(",")));
    }

    // For unions, show member types
    if detail.kind == TypeKind::Union && !detail.union_members.is_empty() {
        out.push_str(&format!("<{}>", detail.union_members.join(",")));
    }

    // Fields
    if !detail.fields.is_empty() {
        out.push(':');
        let field_strs: Vec<String> = detail
            .fields
            .iter()
            .map(|f| {
                let prefix = if f.is_deprecated { "~" } else { "" };
                format!("{}{}:{}", prefix, f.name, abbreviate_type(&f.return_type))
            })
            .collect();
        out.push_str(&field_strs.join(","));
    }

    // Enum values
    if !detail.enum_values.is_empty() {
        out.push(':');
        let vals: Vec<String> = detail
            .enum_values
            .iter()
            .map(|v| {
                if v.is_deprecated {
                    format!("~{}", v.name)
                } else {
                    v.name.clone()
                }
            })
            .collect();
        out.push_str(&vals.join(","));
    }

    out.push('\n');

    // Expanded types
    for expanded in &detail.expanded_types {
        format_expanded_compact(&mut out, expanded);
    }

    out.trim_end().to_string()
}

fn format_field_detail_compact(detail: &FieldDetail) -> String {
    let mut out = String::new();

    // Field line
    out.push_str(&format!(
        "{}.{}({}):{}",
        detail.type_name,
        detail.field_name,
        detail
            .args
            .iter()
            .map(|a| format!("{}:{}", a.name, abbreviate_type(&a.arg_type)))
            .collect::<Vec<_>>()
            .join(","),
        abbreviate_type(&detail.return_type)
    ));
    out.push('\n');

    // Input expansions
    for input in &detail.input_expansions {
        format_expanded_compact(&mut out, input);
    }

    // Return expansion
    if let Some(ret) = &detail.return_expansion {
        format_expanded_compact(&mut out, ret);
    }

    // Via path
    if !detail.via.is_empty() {
        out.push_str(&format!("\u{21b3} {}\n", detail.via[0]));
    }

    out.trim_end().to_string()
}

/// Format search results in compact notation.
#[cfg(feature = "search")]
pub fn format_search_compact(results: &[SearchResult]) -> String {
    let mut out = String::new();

    for result in results {
        out.push_str(&format!(
            "\u{2500}\u{2500} {} \u{2500}\u{2500}\n",
            result.path_header
        ));
        for expanded in &result.types {
            format_expanded_compact(&mut out, expanded);
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

fn format_expanded_compact(out: &mut String, expanded: &ExpandedType) {
    let prefix = kind_prefix(expanded.kind);
    out.push_str(prefix);
    out.push_str(&expanded.name);

    if expanded.kind == TypeKind::Interface && !expanded.union_members.is_empty() {
        out.push_str(&format!("<{}>", expanded.union_members.join(",")));
    }

    if !expanded.fields.is_empty() {
        out.push(':');
        let field_strs: Vec<String> = expanded
            .fields
            .iter()
            .map(|f| {
                let prefix = if f.is_deprecated { "~" } else { "" };
                format!("{}{}:{}", prefix, f.name, abbreviate_type(&f.return_type))
            })
            .collect();
        out.push_str(&field_strs.join(","));
    }

    if !expanded.enum_values.is_empty() {
        out.push(':');
        let vals: Vec<String> = expanded
            .enum_values
            .iter()
            .map(|v| {
                if v.is_deprecated {
                    format!("~{}", v.name)
                } else {
                    v.name.clone()
                }
            })
            .collect();
        out.push_str(&vals.join(","));
    }

    out.push('\n');
}

const fn kind_prefix(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Object => "T:",
        TypeKind::Input => "I:",
        TypeKind::Enum => "E:",
        TypeKind::Interface => "F:",
        TypeKind::Union => "U:",
        TypeKind::Scalar => "S:",
    }
}

/// Abbreviate common scalar types for compact output.
pub fn abbreviate_type(type_str: &str) -> String {
    type_str
        .replace("String", "s")
        .replace("Int", "i")
        .replace("Float", "f")
        .replace("Boolean", "b")
        .replace("ID", "d")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviate_scalars() {
        assert_eq!(abbreviate_type("String"), "s");
        assert_eq!(abbreviate_type("String!"), "s!");
        assert_eq!(abbreviate_type("[String!]!"), "[s!]!");
        assert_eq!(abbreviate_type("Int"), "i");
        assert_eq!(abbreviate_type("Float"), "f");
        assert_eq!(abbreviate_type("Boolean!"), "b!");
        assert_eq!(abbreviate_type("ID!"), "d!");
    }

    #[test]
    fn abbreviate_preserves_custom_types() {
        assert_eq!(abbreviate_type("Post!"), "Post!");
        assert_eq!(abbreviate_type("[User!]!"), "[User!]!");
    }

    #[test]
    fn kind_prefixes() {
        assert_eq!(kind_prefix(TypeKind::Object), "T:");
        assert_eq!(kind_prefix(TypeKind::Input), "I:");
        assert_eq!(kind_prefix(TypeKind::Enum), "E:");
        assert_eq!(kind_prefix(TypeKind::Interface), "F:");
        assert_eq!(kind_prefix(TypeKind::Union), "U:");
    }

    #[test]
    fn deprecated_field_tilde_prefix() {
        let detail = TypeDetail {
            name: "Post".into(),
            kind: TypeKind::Object,
            description: None,
            field_count: 2,
            deprecated_count: 1,
            implements: Vec::new(),
            fields: vec![
                crate::describe::FieldInfo {
                    name: "title".into(),
                    return_type: "String!".into(),
                    description: None,
                    is_deprecated: false,
                    deprecation_reason: None,
                    arg_count: 0,
                },
                crate::describe::FieldInfo {
                    name: "oldSlug".into(),
                    return_type: "String".into(),
                    description: None,
                    is_deprecated: true,
                    deprecation_reason: Some("Use slug instead".into()),
                    arg_count: 0,
                },
            ],
            enum_values: Vec::new(),
            union_members: Vec::new(),
            via: Vec::new(),
            expanded_types: Vec::new(),
        };
        let output = format_type_detail_compact(&detail);
        assert!(output.contains("title:s!"));
        assert!(output.contains("~oldSlug:s"));
    }

    #[test]
    fn deprecated_enum_value_tilde_prefix() {
        let detail = TypeDetail {
            name: "SortOrder".into(),
            kind: TypeKind::Enum,
            description: None,
            field_count: 4,
            deprecated_count: 1,
            implements: Vec::new(),
            fields: Vec::new(),
            enum_values: vec![
                crate::describe::EnumValueInfo {
                    name: "NEWEST".into(),
                    description: None,
                    is_deprecated: false,
                    deprecation_reason: None,
                },
                crate::describe::EnumValueInfo {
                    name: "RELEVANCE".into(),
                    description: None,
                    is_deprecated: true,
                    deprecation_reason: Some("Use TOP instead".into()),
                },
            ],
            union_members: Vec::new(),
            via: Vec::new(),
            expanded_types: Vec::new(),
        };
        let output = format_type_detail_compact(&detail);
        assert!(output.contains("NEWEST"));
        assert!(output.contains("~RELEVANCE"));
    }

    #[test]
    fn expanded_deprecated_field_tilde_prefix() {
        let expanded = ExpandedType {
            name: "User".into(),
            kind: TypeKind::Object,
            fields: vec![
                crate::describe::FieldInfo {
                    name: "id".into(),
                    return_type: "ID!".into(),
                    description: None,
                    is_deprecated: false,
                    deprecation_reason: None,
                    arg_count: 0,
                },
                crate::describe::FieldInfo {
                    name: "legacyId".into(),
                    return_type: "String".into(),
                    description: None,
                    is_deprecated: true,
                    deprecation_reason: None,
                    arg_count: 0,
                },
            ],
            enum_values: Vec::new(),
            union_members: Vec::new(),
            implements: Vec::new(),
        };
        let mut out = String::new();
        format_expanded_compact(&mut out, &expanded);
        assert!(out.contains("id:d!"));
        assert!(out.contains("~legacyId:s"));
    }
}
