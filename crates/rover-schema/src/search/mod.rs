pub mod index;
pub mod tokenizer;

use apollo_compiler::{Schema, schema::ExtendedType};

use self::index::{ElementType, IndexedElement, SchemaIndex};
use crate::{
    describe::{self, ExpandedType},
    error::SchemaError,
    format::ARROW,
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
                    "{} {ARROW} {}.{}",
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

#[cfg(test)]
mod tests {
    use apollo_compiler::Schema;

    use super::*;
    use crate::search::index::ElementType;

    fn test_schema() -> Schema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        match Schema::parse(sdl, "test.graphql") {
            Ok(s) => s,
            Err(e) => e.partial,
        }
    }

    #[test]
    fn search_finds_type_by_name() {
        let schema = test_schema();
        let results = search(&schema, "post", 5, false).unwrap();
        assert!(
            results.iter().any(|r| r.matched_type == "Post"),
            "should find Post type: {results:?}"
        );
    }

    #[test]
    fn search_finds_by_field_name() {
        let schema = test_schema();
        let results = search(&schema, "email", 5, false).unwrap();
        assert!(
            results
                .iter()
                .any(|r| r.matched_type == "User" || r.matched_field.as_deref() == Some("email")),
            "should find User/email: {results:?}"
        );
    }

    #[test]
    fn extract_elements_filters_builtins() {
        let schema = test_schema();
        let elements = extract_elements(&schema, false);
        let builtin_names = ["String", "Int", "Float", "Boolean", "ID"];
        for elem in &elements {
            assert!(
                !builtin_names.contains(&elem.type_name.as_str()),
                "should not contain builtin scalar: {}",
                elem.type_name
            );
            assert!(
                !elem.type_name.starts_with("__"),
                "should not contain introspection type: {}",
                elem.type_name
            );
        }
    }

    #[test]
    fn extract_elements_includes_all_kinds() {
        let schema = test_schema();
        let elements = extract_elements(&schema, false);
        let types: Vec<ElementType> = elements.iter().map(|e| e.element_type).collect();
        assert!(
            types.contains(&ElementType::Type),
            "should have Type elements"
        );
        assert!(
            types.contains(&ElementType::Field),
            "should have Field elements"
        );
        assert!(
            types.contains(&ElementType::EnumValue),
            "should have EnumValue elements"
        );
    }

    #[test]
    fn extract_elements_deprecated_filtering() {
        let schema = test_schema();

        // Without deprecated: should not include oldSlug or legacyId
        let elements = extract_elements(&schema, false);
        let field_names: Vec<&str> = elements
            .iter()
            .filter_map(|e| e.field_name.as_deref())
            .collect();
        assert!(
            !field_names.contains(&"oldSlug"),
            "should exclude deprecated oldSlug"
        );
        assert!(
            !field_names.contains(&"legacyId"),
            "should exclude deprecated legacyId"
        );

        // With deprecated: should include them
        let elements = extract_elements(&schema, true);
        let field_names: Vec<&str> = elements
            .iter()
            .filter_map(|e| e.field_name.as_deref())
            .collect();
        assert!(
            field_names.contains(&"oldSlug"),
            "should include deprecated oldSlug"
        );
        assert!(
            field_names.contains(&"legacyId"),
            "should include deprecated legacyId"
        );
    }

    #[test]
    fn is_root_type_works() {
        let schema = test_schema();
        assert!(is_root_type(&schema, "Query"));
        assert!(is_root_type(&schema, "Mutation"));
        assert!(!is_root_type(&schema, "Post"));
        assert!(!is_root_type(&schema, "User"));
    }

    #[test]
    fn search_deduplicates_paths() {
        let schema = test_schema();
        let results = search(&schema, "post", 10, false).unwrap();
        let headers: Vec<&str> = results.iter().map(|r| r.path_header.as_str()).collect();
        let unique: std::collections::HashSet<&&str> = headers.iter().collect();
        assert_eq!(
            headers.len(),
            unique.len(),
            "should have no duplicate path_header values"
        );
    }

    #[test]
    fn search_respects_limit() {
        let schema = test_schema();
        let results = search(&schema, "post", 1, false).unwrap();
        assert!(results.len() <= 1, "should respect limit=1");
    }

    /// End-to-end: parse SDL → search → extract SDL for matched types.
    /// Exercises the full pipeline that `search --sdl` uses.
    #[test]
    fn search_sdl_end_to_end() {
        use crate::format::sdl::extract_type_sdl;

        let sdl_string = include_str!("../test_fixtures/test_schema.graphql");
        let parsed = crate::ParsedSchema::parse(sdl_string);
        let schema = parsed.inner();

        let results = search(schema, "post", 5, false).unwrap();
        assert!(!results.is_empty(), "should have search results");

        // Replicate the search --sdl output logic
        let mut seen = std::collections::HashSet::new();
        let mut sdl_blocks = Vec::new();
        for result in &results {
            for expanded in &result.types {
                if seen.insert(&expanded.name) {
                    sdl_blocks.push(extract_type_sdl(&expanded.name, sdl_string));
                }
            }
        }
        let output = sdl_blocks.join("\n\n");

        // Should contain actual SDL definitions, not "not found" messages
        assert!(
            !output.contains("# Type '"),
            "should not have not-found markers: {output}"
        );
        // Should contain the Post type definition
        assert!(
            output.contains("type Post"),
            "should include Post SDL: {output}"
        );
        // Should contain at least one other type from the path (e.g. Query)
        assert!(
            output.contains("type Query"),
            "should include Query SDL from the path: {output}"
        );
        // Each type should appear only once
        let post_count = output.matches("type Post").count();
        assert_eq!(post_count, 1, "Post should appear exactly once");
    }
}
