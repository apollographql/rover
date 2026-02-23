pub mod index;
pub mod tokenizer;

use apollo_compiler::{Schema, schema::ExtendedType};

use self::index::{ElementType, IndexedElement, SchemaIndex};
use crate::{
    describe::{self, ExpandedType},
    error::SchemaError,
    root_paths,
};

/// A single search result: a path from root to match with expanded types.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub path_header: String,
    pub types: Vec<ExpandedType>,
    pub matched_type: String,
    pub matched_field: Option<String>,
}

/// Search a schema for the given terms.
pub fn search(
    schema: &Schema,
    query: &str,
    limit: usize,
    include_deprecated: bool,
) -> Result<Vec<SearchResult>, SchemaError> {
    // Build index
    let elements = extract_elements(schema, include_deprecated);
    let index = SchemaIndex::build(elements)?;

    // Search
    let matches = index.search(query, limit * 3)?; // Over-fetch to account for dedup

    // Build paths for each match
    let mut results = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    for matched in &matches {
        let target_type = &matched.type_name;
        let paths = root_paths::find_root_paths(schema, target_type);

        if paths.is_empty() {
            // Type might be a root type itself or unreachable
            if is_root_type(schema, target_type) {
                let path_header = format_root_match(target_type, &matched.field_name);
                if seen_paths.contains(&path_header) {
                    continue;
                }
                seen_paths.insert(path_header.clone());

                let types = build_result_types(schema, &[], target_type, include_deprecated);
                results.push(SearchResult {
                    path_header,
                    types,
                    matched_type: target_type.clone(),
                    matched_field: matched.field_name.clone(),
                });
            }
            continue;
        }

        for path in &paths {
            let path_header = if let Some(field) = &matched.field_name {
                format!(
                    "{} \u{2192} {}.{}",
                    path.format_path_header(target_type),
                    target_type,
                    field
                )
            } else {
                path.format_path_header(target_type)
            };

            if seen_paths.contains(&path_header) {
                continue;
            }
            seen_paths.insert(path_header.clone());

            let types = build_result_types(schema, &path.segments, target_type, include_deprecated);
            results.push(SearchResult {
                path_header,
                types,
                matched_type: target_type.clone(),
                matched_field: matched.field_name.clone(),
            });

            if results.len() >= limit {
                break;
            }
        }

        if results.len() >= limit {
            break;
        }
    }

    results.truncate(limit);
    Ok(results)
}

fn extract_elements(schema: &Schema, include_deprecated: bool) -> Vec<IndexedElement> {
    let mut elements = Vec::new();
    let builtin_scalars = ["String", "Int", "Float", "Boolean", "ID"];

    for (name, ty) in &schema.types {
        let name_str = name.to_string();
        if name_str.starts_with("__") || builtin_scalars.contains(&name_str.as_str()) {
            continue;
        }

        // Index the type itself
        let type_desc = match ty {
            ExtendedType::Object(obj) => obj.description.as_ref().map(|d| d.to_string()),
            ExtendedType::Interface(iface) => iface.description.as_ref().map(|d| d.to_string()),
            ExtendedType::InputObject(inp) => inp.description.as_ref().map(|d| d.to_string()),
            ExtendedType::Enum(e) => e.description.as_ref().map(|d| d.to_string()),
            ExtendedType::Union(u) => u.description.as_ref().map(|d| d.to_string()),
            ExtendedType::Scalar(s) => s.description.as_ref().map(|d| d.to_string()),
        };

        elements.push(IndexedElement {
            element_type: ElementType::Type,
            type_name: name_str.clone(),
            field_name: None,
            description: type_desc,
        });

        // Index fields
        let fields: Vec<(&str, Option<String>, bool)> = match ty {
            ExtendedType::Object(obj) => obj
                .fields
                .iter()
                .map(|(n, f)| {
                    (
                        n.as_str(),
                        f.description.as_ref().map(|d| d.to_string()),
                        f.directives.get("deprecated").is_some(),
                    )
                })
                .collect(),
            ExtendedType::Interface(iface) => iface
                .fields
                .iter()
                .map(|(n, f)| {
                    (
                        n.as_str(),
                        f.description.as_ref().map(|d| d.to_string()),
                        f.directives.get("deprecated").is_some(),
                    )
                })
                .collect(),
            ExtendedType::InputObject(inp) => inp
                .fields
                .iter()
                .map(|(n, f)| {
                    (
                        n.as_str(),
                        f.description.as_ref().map(|d| d.to_string()),
                        f.directives.get("deprecated").is_some(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };

        for (field_name, desc, is_deprecated) in fields {
            if !include_deprecated && is_deprecated {
                continue;
            }
            elements.push(IndexedElement {
                element_type: ElementType::Field,
                type_name: name_str.clone(),
                field_name: Some(field_name.to_string()),
                description: desc,
            });
        }

        // Index enum values
        if let ExtendedType::Enum(e) = ty {
            for (val_name, val) in &e.values {
                let is_deprecated = val.directives.get("deprecated").is_some();
                if !include_deprecated && is_deprecated {
                    continue;
                }
                elements.push(IndexedElement {
                    element_type: ElementType::EnumValue,
                    type_name: name_str.clone(),
                    field_name: Some(val_name.to_string()),
                    description: val.description.as_ref().map(|d| d.to_string()),
                });
            }
        }
    }

    elements
}

fn is_root_type(schema: &Schema, type_name: &str) -> bool {
    let query_name = schema
        .schema_definition
        .query
        .as_ref()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Query".to_string());
    let mutation_name = schema
        .schema_definition
        .mutation
        .as_ref()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Mutation".to_string());

    type_name == query_name || type_name == mutation_name
}

fn format_root_match(type_name: &str, field_name: &Option<String>) -> String {
    if let Some(field) = field_name {
        format!("{type_name}.{field}")
    } else {
        type_name.to_string()
    }
}

fn build_result_types(
    schema: &Schema,
    path_segments: &[root_paths::PathSegment],
    target_type: &str,
    include_deprecated: bool,
) -> Vec<ExpandedType> {
    let mut types = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Add intermediate types from the path (depth 1 — just their fields)
    for seg in path_segments {
        if seen.contains(&seg.type_name) {
            continue;
        }
        seen.insert(seg.type_name.clone());
        if let Some(expanded) =
            describe::expand_single_type(schema, &seg.type_name, include_deprecated)
        {
            types.push(expanded);
        }
    }

    // Add the target type
    if !seen.contains(target_type)
        && let Some(expanded) =
            describe::expand_single_type(schema, target_type, include_deprecated)
    {
        types.push(expanded);
    }

    types
}
