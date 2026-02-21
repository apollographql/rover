use apollo_compiler::Schema;

use crate::coordinate::SchemaCoordinate;

/// Extract filtered SDL for a coordinate from the full schema SDL.
/// Returns SDL containing just the targeted type (and for fields, just the parent type).
pub fn filtered_sdl(schema: &Schema, coord: Option<&SchemaCoordinate>, sdl: &str) -> String {
    match coord {
        None => sdl.to_string(),
        Some(coord) => {
            let type_name = coord.type_name();
            // Simple approach: re-serialize just the targeted type from the parsed schema
            extract_type_sdl(schema, type_name, sdl)
        }
    }
}

fn extract_type_sdl(_schema: &Schema, type_name: &str, full_sdl: &str) -> String {
    // Find the type definition in the original SDL text
    // Look for patterns like: type TypeName, input TypeName, enum TypeName, etc.
    let patterns = [
        format!("type {type_name}"),
        format!("input {type_name}"),
        format!("enum {type_name}"),
        format!("interface {type_name}"),
        format!("union {type_name}"),
        format!("scalar {type_name}"),
    ];

    for pattern in &patterns {
        if let Some(start) = full_sdl.find(pattern.as_str()) {
            // Find the end of this type definition
            if let Some(end) = find_type_end(full_sdl, start) {
                return full_sdl[start..end].trim().to_string();
            }
        }
    }

    // Fallback: check if we have a description comment before the type
    for pattern in &patterns {
        // Look for """ description before the type
        if let Some(type_pos) = full_sdl.find(pattern.as_str()) {
            let before = &full_sdl[..type_pos];
            let desc_start = before.rfind("\"\"\"").unwrap_or(type_pos);
            if let Some(end) = find_type_end(full_sdl, type_pos) {
                return full_sdl[desc_start..end].trim().to_string();
            }
        }
    }

    format!("# Type '{type_name}' not found in SDL")
}

fn find_type_end(sdl: &str, start: usize) -> Option<usize> {
    let rest = &sdl[start..];

    // For types with body: find the matching closing brace
    if let Some(open) = rest.find('{') {
        let mut depth = 0;
        for (i, ch) in rest[open..].char_indices() {
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

    // For scalars and unions without braces: find next newline after content
    if let Some(nl) = rest.find('\n') {
        // Check if there's a brace somewhere on the same line
        let line = &rest[..nl];
        if !line.contains('{') {
            return Some(start + nl);
        }
    }

    Some(sdl.len())
}
