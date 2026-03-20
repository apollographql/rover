use apollo_compiler::{Schema, coordinate::SchemaCoordinate};

/// Returns SDL for the schema, optionally filtered to the type referenced by `coord`.
///
/// - `None` — returns the full schema SDL
/// - `Type` / `TypeAttribute` / `FieldArgument` — returns the SDL for the parent type definition
/// - `Directive` / `DirectiveArgument` — returns the full schema SDL (no single-item extraction)
pub fn filtered_sdl(coord: Option<&SchemaCoordinate>, schema: &Schema) -> String {
    let Some(coord) = coord else {
        return schema.serialize().to_string();
    };

    let type_name = match coord {
        SchemaCoordinate::Type(tc) => &tc.ty,
        SchemaCoordinate::TypeAttribute(tac) => &tac.ty,
        SchemaCoordinate::FieldArgument(fac) => &fac.ty,
        SchemaCoordinate::Directive(_) | SchemaCoordinate::DirectiveArgument(_) => {
            return schema.serialize().to_string();
        }
    };

    schema
        .types
        .get(type_name)
        .map(|ty| ty.serialize().to_string())
        .unwrap_or_else(|| format!("# Type '{type_name}' not found in SDL"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(sdl: &str) -> Schema {
        match Schema::parse(sdl, "schema.graphql") {
            Ok(schema) => schema,
            Err(with_errors) => with_errors.partial,
        }
    }

    const TEST_SDL: &str = r#""""A content post"""
type Post implements Node & Timestamped {
  id: ID!
  title: String!
  body: String!
}

type PostEdge {
  node: Post!
}

input CreatePostInput {
  title: String!
  body: String!
}

"""Content search types"""
enum SearchType {
  POST
  USER
  CATEGORY
}

"""An entity with a stable ID"""
interface Node {
  id: ID!
}

union ContentItem = Post | PostEdge

scalar DateTime
"#;

    #[test]
    fn filtered_sdl_none_returns_full() {
        let schema = parse(TEST_SDL);
        let result = filtered_sdl(None, &schema);
        assert!(result.contains("type Post"));
        assert!(result.contains("type PostEdge"));
        assert!(result.contains("scalar DateTime"));
    }

    #[test]
    fn extract_object_type() {
        let schema = parse(TEST_SDL);
        let coord = "PostEdge".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("type PostEdge"), "got: {result}");
        assert!(result.contains("node: Post!"));
        assert!(
            !result.contains("type Post "),
            "must not include other types"
        );
    }

    #[test]
    fn extract_input_type() {
        let schema = parse(TEST_SDL);
        let coord = "CreatePostInput".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("input CreatePostInput"), "got: {result}");
        assert!(result.contains("title: String!"));
    }

    #[test]
    fn extract_enum_type() {
        let schema = parse(TEST_SDL);
        let coord = "SearchType".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("enum SearchType"), "got: {result}");
        assert!(result.contains("POST"));
    }

    #[test]
    fn extract_interface_type() {
        let schema = parse(TEST_SDL);
        let coord = "Node".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("interface Node"), "got: {result}");
        assert!(result.contains("id: ID!"));
    }

    #[test]
    fn extract_union_type() {
        let schema = parse(TEST_SDL);
        let coord = "ContentItem".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("union ContentItem"), "got: {result}");
    }

    #[test]
    fn extract_scalar_type() {
        let schema = parse(TEST_SDL);
        let coord = "DateTime".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("scalar DateTime"), "got: {result}");
    }

    #[test]
    fn extract_type_via_field_coordinate() {
        let schema = parse(TEST_SDL);
        let coord = "Post.title".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert!(result.contains("type Post"), "got: {result}");
        assert!(result.contains("title: String!"));
    }

    #[test]
    fn type_not_found_fallback() {
        let schema = parse(TEST_SDL);
        let coord = "NonExistent".parse::<SchemaCoordinate>().unwrap();
        let result = filtered_sdl(Some(&coord), &schema);
        assert_eq!(result, "# Type 'NonExistent' not found in SDL");
    }
}
