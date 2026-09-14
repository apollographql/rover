/// Extracts the contiguous block of comment lines immediately preceding the
/// definition that starts at `definition_offset` in `source`.
///
/// Any non-comment line, including a blank one, ends the block. Comment lines
/// are returned in source order with surrounding whitespace trimmed, so the
/// result is stable regardless of how the source was indented.
///
/// GraphQL treats comments as ignored tokens, so `apollo-compiler` discards
/// them during parsing and they cannot be recovered from the AST. Reading them
/// back out of the original source text is the only option, which is why this
/// works on byte offsets rather than on parsed nodes.
pub(super) fn extract_leading_comments(source: &str, definition_offset: usize) -> Option<String> {
    let definition_line_start = source[..definition_offset]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);

    // Walk backwards over the run of comment lines directly above the
    // definition, then restore top-to-bottom order.
    let mut lines = Vec::new();
    let mut cursor = definition_line_start;
    while cursor > 0 {
        let line_start = source[..cursor - 1]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let line = source[line_start..cursor].trim();
        if !line.starts_with('#') {
            break;
        }
        lines.push(line);
        cursor = line_start;
    }

    if lines.is_empty() {
        return None;
    }

    lines.reverse();
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::extract_leading_comments;

    fn extract(source: &str) -> Option<String> {
        let offset = source
            .find("query ")
            .expect("test source must contain an operation");
        extract_leading_comments(source, offset)
    }

    #[test]
    fn single_comment_line_is_extracted() {
        assert_that!(extract("# @meta fci_tier: TIER_1\nquery GetFoo { id }"))
            .is_equal_to(Some("# @meta fci_tier: TIER_1".to_string()));
    }

    #[test]
    fn contiguous_comment_lines_keep_source_order() {
        assert_that!(extract(
            "# first\n# @meta fci_tier: TIER_1\n# last\nquery GetFoo { id }"
        ))
        .is_equal_to(Some(
            "# first\n# @meta fci_tier: TIER_1\n# last".to_string(),
        ));
    }

    #[test]
    fn blank_line_ends_the_block() {
        assert_that!(extract("# detached\n\nquery GetFoo { id }")).is_none();
    }

    #[test]
    fn preceding_definition_ends_the_block() {
        assert_that!(extract(
            "# on the fragment\nfragment F on Product { id }\nquery GetFoo { ...F }"
        ))
        .is_none();
    }

    #[test]
    fn indentation_is_normalized() {
        assert_that!(extract("    # @meta fci_tier: TIER_1\nquery GetFoo { id }"))
            .is_equal_to(Some("# @meta fci_tier: TIER_1".to_string()));
    }

    #[test]
    fn carriage_returns_are_trimmed() {
        assert_that!(extract("# @meta fci_tier: TIER_1\r\nquery GetFoo { id }"))
            .is_equal_to(Some("# @meta fci_tier: TIER_1".to_string()));
    }

    #[test]
    fn comment_on_the_first_line_is_extracted() {
        assert_that!(extract("#@meta fci_tier: TIER_1\nquery GetFoo { id }"))
            .is_equal_to(Some("#@meta fci_tier: TIER_1".to_string()));
    }

    #[test]
    fn definition_on_the_first_line_has_no_comments() {
        assert_that!(extract("query GetFoo { id }")).is_none();
    }

    #[test]
    fn trailing_comment_from_a_previous_operation_is_not_captured() {
        let source = "query GetBar { id } # trailing\nquery GetFoo { id }";
        let offset = source.rfind("query ").unwrap();
        assert_that!(extract_leading_comments(source, offset)).is_none();
    }
}
