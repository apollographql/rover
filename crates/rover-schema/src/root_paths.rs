use std::hash::{Hash, Hasher};

use apollo_compiler::{Name, ast::OperationType, schema::ExtendedType};
use pathfinding::prelude::bfs;

use crate::ParsedSchema;

/// A path from a root type (Query/Mutation) to a target type through field references.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RootPath {
    /// Sequence of (type_name, field_name) pairs from root to target.
    /// The last element's type_name is the parent of the target.
    pub(crate) segments: Vec<PathSegment>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PathSegment {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
}

/// Schema path node carrying the field name used to arrive at `current`.
/// Equality and hashing are based solely on `current` so the visited set
/// deduplicates by type name regardless of how a type was reached.
#[derive(Clone)]
struct SchemaPathNode {
    from: Option<Name>,
    current: Name,
}

impl PartialEq for SchemaPathNode {
    fn eq(&self, other: &Self) -> bool {
        self.current == other.current
    }
}

impl Eq for SchemaPathNode {}

impl Hash for SchemaPathNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.current.hash(state);
    }
}

impl ParsedSchema {
    /// Find the shortest path(s) from Query/Mutation/Subscription roots to a target type.
    pub fn find_root_paths(&self, target_type: &Name) -> Vec<RootPath> {
        let schema = &self.inner();
        [
            OperationType::Query,
            OperationType::Mutation,
            OperationType::Subscription,
        ]
        .into_iter()
        .filter_map(|op| schema.root_operation(op))
        .filter(|root_name| *root_name != target_type)
        .filter_map(|root_name| {
            let start = SchemaPathNode {
                from: None,
                current: root_name.clone(),
            };
            let path = bfs(
                &start,
                |node| self.successors(node),
                |node| node.current == *target_type,
            )?;
            Some(path_to_root_path(path))
        })
        .collect()
    }
    fn successors(&self, node: &SchemaPathNode) -> Vec<SchemaPathNode> {
        let schema = self.inner();
        let ty = match schema.types.get(node.current.as_str()) {
            Some(ty) => ty,
            None => return Vec::new(),
        };
        let fields = match ty {
            ExtendedType::Object(obj) => &obj.fields,
            ExtendedType::Interface(iface) => &iface.fields,
            _ => return Vec::new(),
        };
        fields
            .iter()
            .map(|(field_name, field)| SchemaPathNode {
                from: Some(field_name.clone()),
                current: field.ty.inner_named_type().clone(),
            })
            .collect()
    }
}

fn path_to_root_path(path: Vec<SchemaPathNode>) -> RootPath {
    let segments = path
        .windows(2)
        .map(|w| PathSegment {
            type_name: w[0].current.clone(),
            field_name: w[1]
                .from
                .clone()
                .expect("non-root node always has a field name"),
        })
        .collect();
    RootPath { segments }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;
    use crate::ParsedSchema;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[rstest]
    fn finds_direct_path_from_query(schema: ParsedSchema) {
        let paths = schema.find_root_paths(&Name::new("Post").unwrap());
        assert_that!(paths).is_not_empty();
        let has_direct = paths.iter().any(|p| {
            p.segments.len() == 1
                && p.segments[0].type_name == "Query"
                && p.segments[0].field_name == "post"
        });
        assert_that!(has_direct).is_true();
    }

    #[rstest]
    fn finds_path_to_nested_type(schema: ParsedSchema) {
        let paths = schema.find_root_paths(&Name::new("Preferences").unwrap());
        assert_that!(paths).has_length(2);

        let query_path = paths
            .iter()
            .find(|p| p.segments[0].type_name == "Query")
            .unwrap();
        assert_that!(query_path.segments).has_length(2);
        assert_that!(query_path.segments[0].type_name).is_equal_to(Name::new("Query").unwrap());
        assert_that!(query_path.segments[0].field_name).is_equal_to(Name::new("viewer").unwrap());
        assert_that!(query_path.segments[1].type_name).is_equal_to(Name::new("Viewer").unwrap());
        assert_that!(query_path.segments[1].field_name)
            .is_equal_to(Name::new("preferences").unwrap());

        let mutation_path = paths
            .iter()
            .find(|p| p.segments[0].type_name == "Mutation")
            .unwrap();
        assert_that!(mutation_path.segments).has_length(2);
        assert_that!(mutation_path.segments[0].type_name)
            .is_equal_to(Name::new("Mutation").unwrap());
        assert_that!(mutation_path.segments[0].field_name)
            .is_equal_to(Name::new("updatePreferences").unwrap());
        assert_that!(mutation_path.segments[1].type_name)
            .is_equal_to(Name::new("UpdatePreferencesPayload").unwrap());
        assert_that!(mutation_path.segments[1].field_name)
            .is_equal_to(Name::new("preferences").unwrap());
    }

    #[rstest]
    fn no_path_for_root_type(schema: ParsedSchema) {
        let paths = schema.find_root_paths(&Name::new("Query").unwrap());
        assert_that!(paths).is_empty();
    }

    #[rstest]
    fn no_path_for_unreachable_scalar(schema: ParsedSchema) {
        let paths = schema.find_root_paths(&Name::new("URL").unwrap());
        assert_that!(paths).is_empty();
    }
}
