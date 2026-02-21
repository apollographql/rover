use std::collections::{HashMap, HashSet, VecDeque};

use apollo_compiler::{Schema, schema::ExtendedType};

/// A path from a root type (Query/Mutation) to a target type through field references.
#[derive(Debug, Clone)]
pub struct RootPath {
    /// Sequence of (type_name, field_name) pairs from root to target.
    /// The last element's type_name is the parent of the target.
    pub segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub type_name: String,
    pub field_name: String,
}

impl RootPath {
    pub fn format_via(&self) -> String {
        self.segments
            .iter()
            .map(|seg| format!("{}.{}", seg.type_name, seg.field_name))
            .collect::<Vec<_>>()
            .join(" \u{203a} ")
    }

    pub fn format_compact(&self) -> String {
        self.segments
            .iter()
            .map(|seg| format!("{}.{}", seg.type_name, seg.field_name))
            .collect::<Vec<_>>()
            .join("\u{2192}")
    }

    pub fn format_path_header(&self, target_type: &str) -> String {
        let mut parts: Vec<String> = self
            .segments
            .iter()
            .map(|seg| format!("{}.{}", seg.type_name, seg.field_name))
            .collect();
        parts.push(target_type.to_string());
        parts.join(" \u{2192} ")
    }
}

/// Find the shortest BFS path(s) from Query/Mutation roots to a target type.
pub fn find_root_paths(schema: &Schema, target_type: &str) -> Vec<RootPath> {
    let mut paths = Vec::new();

    for root_name in root_type_names(schema) {
        if root_name == target_type {
            continue;
        }
        if let Some(path) = bfs_to_type(schema, &root_name, target_type) {
            paths.push(path);
        }
    }

    paths
}

fn root_type_names(schema: &Schema) -> Vec<String> {
    let mut roots = Vec::new();
    if let Some(query) = schema.schema_definition.query.as_ref() {
        roots.push(query.to_string());
    } else if schema.types.contains_key("Query") {
        roots.push("Query".to_string());
    }
    if let Some(mutation) = schema.schema_definition.mutation.as_ref() {
        roots.push(mutation.to_string());
    } else if schema.types.contains_key("Mutation") {
        roots.push("Mutation".to_string());
    }
    roots
}

fn bfs_to_type(schema: &Schema, start: &str, target: &str) -> Option<RootPath> {
    let mut visited: HashSet<String> = HashSet::new();
    // Map from type_name -> (parent_type, field_name)
    let mut parent_map: HashMap<String, (String, String)> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    visited.insert(start.to_string());
    queue.push_back(start.to_string());

    const MAX_DEPTH: usize = 10;
    let mut depth = 0;

    while !queue.is_empty() && depth < MAX_DEPTH {
        let level_size = queue.len();
        for _ in 0..level_size {
            let current = queue.pop_front().unwrap();

            if let Some(fields) = get_object_fields(schema, &current) {
                for (field_name, return_type) in fields {
                    let type_name = unwrap_type_name(&return_type);
                    if type_name == target {
                        parent_map
                            .insert(type_name, (current.clone(), field_name));
                        return Some(reconstruct_path(&parent_map, start, target));
                    }
                    if !visited.contains(&type_name) {
                        visited.insert(type_name.clone());
                        parent_map
                            .insert(type_name.clone(), (current.clone(), field_name));
                        queue.push_back(type_name);
                    }
                }
            }
        }
        depth += 1;
    }

    None
}

fn reconstruct_path(
    parent_map: &HashMap<String, (String, String)>,
    start: &str,
    target: &str,
) -> RootPath {
    let mut segments = Vec::new();
    let mut current = target.to_string();

    while current != start {
        if let Some((parent_type, field_name)) = parent_map.get(&current) {
            segments.push(PathSegment {
                type_name: parent_type.clone(),
                field_name: field_name.clone(),
            });
            current = parent_type.clone();
        } else {
            break;
        }
    }

    segments.reverse();
    RootPath { segments }
}

fn get_object_fields(schema: &Schema, type_name: &str) -> Option<Vec<(String, String)>> {
    let ty = schema.types.get(type_name)?;
    match ty {
        ExtendedType::Object(obj) => Some(
            obj.fields
                .iter()
                .map(|(name, field)| (name.to_string(), field.ty.to_string()))
                .collect(),
        ),
        ExtendedType::Interface(iface) => Some(
            iface
                .fields
                .iter()
                .map(|(name, field)| (name.to_string(), field.ty.to_string()))
                .collect(),
        ),
        _ => None,
    }
}

/// Extract the named type from a type reference (strip [], !)
fn unwrap_type_name(type_str: &str) -> String {
    type_str
        .replace(['[', ']', '!'], "")
        .trim()
        .to_string()
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
    fn unwrap_type_name_strips_wrappers() {
        assert_eq!(unwrap_type_name("[String!]!"), "String");
        assert_eq!(unwrap_type_name("Post"), "Post");
        assert_eq!(unwrap_type_name("[User!]"), "User");
    }

    #[test]
    fn finds_direct_path_from_query() {
        let schema = test_schema();
        let paths = find_root_paths(&schema, "Post");
        assert!(!paths.is_empty());
        let has_direct = paths.iter().any(|p| {
            p.segments.len() == 1
                && p.segments[0].type_name == "Query"
                && p.segments[0].field_name == "post"
        });
        assert!(has_direct, "Should find direct Query.post path");
    }

    #[test]
    fn finds_path_to_nested_type() {
        let schema = test_schema();
        let paths = find_root_paths(&schema, "Preferences");
        assert!(!paths.is_empty());
        // Preferences is reachable through Query.viewer → Viewer.preferences
    }

    #[test]
    fn no_path_for_root_type() {
        let schema = test_schema();
        let paths = find_root_paths(&schema, "Query");
        assert!(paths.is_empty());
    }

    #[test]
    fn no_path_for_unreachable_scalar() {
        let schema = test_schema();
        let paths = find_root_paths(&schema, "URL");
        assert!(paths.is_empty());
    }

    #[test]
    fn format_via_produces_readable_output() {
        let path = RootPath {
            segments: vec![
                PathSegment { type_name: "Query".into(), field_name: "user".into() },
                PathSegment { type_name: "User".into(), field_name: "posts".into() },
            ],
        };
        let via = path.format_via();
        assert!(via.contains("Query.user"));
        assert!(via.contains("User.posts"));
    }
}
