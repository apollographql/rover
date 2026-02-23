use crate::coordinate::SchemaCoordinate;

/// Extract filtered SDL for a coordinate from the full schema SDL.
/// Returns SDL containing just the targeted type (and for fields, just the parent type).
pub fn filtered_sdl(coord: Option<&SchemaCoordinate>, sdl: &str) -> String {
    match coord {
        None => sdl.to_string(),
        Some(coord) => extract_type_sdl(coord.type_name(), sdl),
    }
}

fn extract_type_sdl(type_name: &str, full_sdl: &str) -> String {
    let patterns = [
        format!("type {type_name}"),
        format!("input {type_name}"),
        format!("enum {type_name}"),
        format!("interface {type_name}"),
        format!("union {type_name}"),
        format!("scalar {type_name}"),
    ];

    for pattern in &patterns {
        let Some(type_pos) = full_sdl.find(pattern.as_str()) else {
            continue;
        };
        let Some(end) = find_type_end(full_sdl, type_pos) else {
            continue;
        };

        // Include a preceding `"""` description block if present
        let before = &full_sdl[..type_pos];
        let start = before
            .rfind("\"\"\"")
            .filter(|&desc_pos| {
                // Only include if the description block is adjacent (no other type defs between)
                full_sdl[desc_pos..type_pos].trim().ends_with("\"\"\"")
            })
            .unwrap_or(type_pos);

        return full_sdl[start..end].trim().to_string();
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
        let line = &rest[..nl];
        if !line.contains('{') {
            return Some(start + nl);
        }
    }

    Some(sdl.len())
}
