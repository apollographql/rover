use crate::coordinate::SchemaCoordinate;

/// Extract filtered SDL for a coordinate from the full schema SDL.
/// Returns SDL containing just the targeted type (and for fields, just the parent type).
pub fn filtered_sdl(coord: Option<&SchemaCoordinate>, sdl: &str) -> String {
    match coord {
        None => sdl.to_string(),
        Some(coord) => extract_type_sdl(coord.type_name(), sdl),
    }
}

/// Extract the SDL definition for a single type from the full schema SDL.
///
/// Searches for `type Name`, `input Name`, etc. with word-boundary checking
/// to avoid matching `Post` when `PostEdge` appears first.
pub fn extract_type_sdl(type_name: &str, full_sdl: &str) -> String {
    let patterns = [
        format!("type {type_name}"),
        format!("input {type_name}"),
        format!("enum {type_name}"),
        format!("interface {type_name}"),
        format!("union {type_name}"),
        format!("scalar {type_name}"),
    ];

    for pattern in &patterns {
        // Iterate through all occurrences, checking word boundary after the type name
        let mut search_from = 0;
        let type_pos = loop {
            let Some(pos) = full_sdl[search_from..].find(pattern.as_str()) else {
                break None;
            };
            let abs_pos = search_from + pos;
            let next = full_sdl[abs_pos + pattern.len()..].chars().next();
            if next.is_none_or(|c| !c.is_alphanumeric() && c != '_') {
                break Some(abs_pos);
            }
            search_from = abs_pos + 1;
        };
        let Some(type_pos) = type_pos else {
            continue;
        };

        let Some(end) = find_type_end(full_sdl, type_pos) else {
            continue;
        };

        // Include a preceding `"""` description block if present
        let before = &full_sdl[..type_pos];
        let start = before
            .rfind("\"\"\"")
            .filter(|&closing_pos| {
                // Only include if the description block is adjacent (no other type defs between)
                full_sdl[closing_pos..type_pos].trim().ends_with("\"\"\"")
            })
            .and_then(|closing_pos| {
                // Find the opening `"""` before the closing one
                before[..closing_pos].rfind("\"\"\"")
            })
            .unwrap_or(type_pos);

        return full_sdl[start..end].trim().to_string();
    }

    format!("# Type '{type_name}' not found in SDL")
}

fn find_type_end(sdl: &str, start: usize) -> Option<usize> {
    let rest = &sdl[start..];

    // For scalars and unions without braces on the first line: end at the newline
    if let Some(nl) = rest.find('\n') {
        let first_line = &rest[..nl];
        if !first_line.contains('{') {
            return Some(start + nl);
        }
    }

    // For types with body: find the matching closing brace,
    // skipping over triple-quoted (""") string literals.
    if let Some(open) = rest.find('{') {
        let mut depth = 0;
        let body = &rest[open..];
        let mut chars = body.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            if ch == '"' {
                // Check for triple-quote start
                if body[i..].starts_with("\"\"\"") {
                    // Skip the opening """
                    chars.next();
                    chars.next();
                    // Consume until closing """
                    while let Some((j, _)) = chars.next() {
                        if body[j..].starts_with("\"\"\"") {
                            chars.next();
                            chars.next();
                            break;
                        }
                    }
                    continue;
                }
            }
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(start + open + i + 1);
                    }
                }
                _ => {}
            }
        }
    }

    Some(sdl.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SDL: &str = r#""""A content post"""
type Post implements Node & Timestamped {
  id: ID!
  title: String!
  body: String!
}

type PostEdge {
  node: Post!
}

type PostConnection {
  edges: [PostEdge!]
  pageInfo: PageInfo!
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

union ContentItem = Post | Comment

scalar DateTime

"""Email digest frequency"""
enum DigestFrequency {
  DAILY
  WEEKLY
  NEVER
}"#;

    #[test]
    fn extract_object_type() {
        let result = extract_type_sdl("PostEdge", TEST_SDL);
        assert!(result.starts_with("type PostEdge"), "got: {result}");
        assert!(result.contains("node: Post!"));
    }

    #[test]
    fn extract_input_type() {
        let result = extract_type_sdl("CreatePostInput", TEST_SDL);
        assert!(result.starts_with("input CreatePostInput"), "got: {result}");
        assert!(result.contains("title: String!"));
    }

    #[test]
    fn extract_enum_type() {
        let result = extract_type_sdl("SearchType", TEST_SDL);
        assert!(
            result.contains("enum SearchType"),
            "should contain enum: {result}"
        );
        assert!(result.contains("POST"));
    }

    #[test]
    fn extract_interface_type() {
        let result = extract_type_sdl("Node", TEST_SDL);
        assert!(
            result.contains("interface Node"),
            "should contain interface: {result}"
        );
        assert!(result.contains("id: ID!"));
    }

    #[test]
    fn extract_union_type() {
        let result = extract_type_sdl("ContentItem", TEST_SDL);
        assert!(
            result.starts_with("union ContentItem"),
            "should start with union: {result}"
        );
        assert!(result.contains("Post | Comment"));
    }

    #[test]
    fn extract_scalar_type() {
        let result = extract_type_sdl("DateTime", TEST_SDL);
        assert_eq!(result, "scalar DateTime");
    }

    #[test]
    fn extract_type_with_description_block() {
        let result = extract_type_sdl("Post", TEST_SDL);
        assert!(
            result.starts_with("\"\"\"A content post\"\"\""),
            "should include description block: {result}"
        );
        assert!(result.contains("type Post"));
    }

    #[test]
    fn extract_enum_with_description_block() {
        let result = extract_type_sdl("DigestFrequency", TEST_SDL);
        assert!(
            result.starts_with("\"\"\"Email digest frequency\"\"\""),
            "should include description: {result}"
        );
        assert!(result.contains("enum DigestFrequency"));
    }

    #[test]
    fn type_name_prefix_bug_post_vs_postedge() {
        // PostEdge appears after Post in SDL, but searching for "Post" must not match "PostEdge"
        let result = extract_type_sdl("Post", TEST_SDL);
        assert!(
            result.contains("type Post implements"),
            "should find 'type Post', not 'type PostEdge': {result}"
        );
        assert!(
            !result.contains("type PostEdge"),
            "must not contain PostEdge definition: {result}"
        );
    }

    #[test]
    fn type_name_prefix_bug_reversed_order() {
        // SDL where PostEdge appears BEFORE Post — the original bug
        let sdl = "type PostEdge {\n  node: Post!\n}\n\ntype Post {\n  id: ID!\n}\n";
        let result = extract_type_sdl("Post", sdl);
        assert!(
            result.starts_with("type Post"),
            "should find exact 'type Post', not 'type PostEdge': {result}"
        );
        assert!(!result.contains("PostEdge"), "must not match PostEdge");
    }

    #[test]
    fn braces_inside_description_do_not_break_extraction() {
        let sdl = "type Foo {\n  \"\"\"\n  Example: { bar: 1 }\n  \"\"\"\n  field: String!\n}\n\ntype Bar {\n  id: ID!\n}\n";
        let result = extract_type_sdl("Foo", sdl);
        assert!(
            result.contains("field: String!"),
            "should include Foo's field: {result}"
        );
        assert!(
            !result.contains("type Bar"),
            "should not bleed into Bar: {result}"
        );
    }

    #[test]
    fn not_found_fallback() {
        let result = extract_type_sdl("NonExistent", TEST_SDL);
        assert_eq!(result, "# Type 'NonExistent' not found in SDL");
    }

    #[test]
    fn filtered_sdl_none_returns_full() {
        let result = filtered_sdl(None, TEST_SDL);
        assert_eq!(result, TEST_SDL);
    }
}
