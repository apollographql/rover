/// Extracts the contiguous block of comment lines immediately preceding the
/// definition that starts at `definition_offset` in `source`. The scan stops at
/// `previous_definition_end` so text inside an earlier definition is excluded.
///
/// Any non-comment line, including a blank one, ends the block. Comment lines
/// are returned in source order with surrounding whitespace trimmed, so the
/// result is stable regardless of how the source was indented. Byte order marks
/// before a comment are ignored, but those within its text are preserved.
///
/// GraphQL treats comments as ignored tokens, so `apollo-compiler` discards
/// them during parsing and they cannot be recovered from the AST. Reading them
/// back out of the original source text is the only option, which is why this
/// works on byte offsets rather than on parsed nodes.
pub(super) fn extract_leading_comments(
    source: &str,
    definition_offset: usize,
    previous_definition_end: usize,
) -> Option<String> {
    let definition_line_start = source[..definition_offset]
        .rfind(['\r', '\n'])
        .map_or(0, |newline| newline + 1);

    // Do not skip an earlier definition or a block string's closing delimiter.
    // Commas and byte order marks, like spaces and tabs, are ignored by GraphQL.
    if !source[definition_line_start..definition_offset]
        .chars()
        .all(|ch| matches!(ch, ' ' | '\t' | ',' | '\u{feff}'))
    {
        return None;
    }

    // Walk backwards over the run of comment lines directly above the
    // definition, then restore top-to-bottom order.
    let mut lines = Vec::new();
    let mut cursor = definition_line_start;
    while cursor > previous_definition_end {
        // CRLF is one line ending; a lone CR or LF is also a line ending.
        let line_end = if source[..cursor].ends_with("\r\n") {
            cursor - 2
        } else {
            cursor - 1
        };
        let line_start = source[..line_end]
            .rfind(['\r', '\n'])
            .map_or(0, |newline| newline + 1);
        if line_start < previous_definition_end {
            break;
        }
        let line = source[line_start..line_end]
            .trim_start_matches([' ', '\t', '\u{feff}'])
            .trim_end();
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
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::extract_leading_comments;

    fn extract(source: &str) -> Option<String> {
        let offset = source
            .find("query ")
            .expect("test source must contain an operation");
        extract_leading_comments(source, offset, 0)
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
        assert_that!(extract_leading_comments(source, offset, 0)).is_none();
    }

    #[rstest]
    #[case::operation("# A metadata\nquery A { a } query B { b }")]
    #[case::fragment("# Fragment metadata\nfragment F on X { x } query Q { ...F }")]
    #[case::block_string("query A { f(arg: \"\"\"\n# string content\n\"\"\") } query B { b }")]
    fn same_line_definition_prevents_comment_ownership(#[case] source: &str) {
        let offset = source.rfind("query ").unwrap();
        assert_that!(extract_leading_comments(source, offset, 0)).is_none();
    }

    #[rstest]
    #[case::lf("\n")]
    #[case::cr("\r")]
    #[case::crlf("\r\n")]
    fn graphql_line_endings_preserve_contiguous_comments(#[case] newline: &str) {
        let source = format!("# first{newline}\t# last{newline}  query Q {{ x }}");
        assert_that!(extract(&source)).is_equal_to(Some("# first\n# last".to_string()));
    }

    #[rstest]
    #[case::lf("\n")]
    #[case::cr("\r")]
    #[case::crlf("\r\n")]
    fn blank_lines_remain_boundaries_with_every_line_ending(#[case] newline: &str) {
        let source = format!("# detached{newline} \t{newline}query Q {{ x }}");
        assert_that!(extract(&source)).is_none();
    }

    #[rstest]
    #[case::initial_bom("\u{feff}# metadata\nquery Q { x }")]
    #[case::indented_bom(" \t\u{feff} # metadata\nquery Q { x }")]
    #[case::operation_bom("# metadata\n\u{feff}\tquery Q { x }")]
    #[case::later_bom("fragment F on X { x }\n\u{feff}# metadata\nquery Q { x }")]
    #[case::ignored_comma_before_operation("# metadata\n, \t\u{feff}query Q { x }")]
    fn ignored_prefixes_do_not_hide_comments(#[case] source: &str) {
        assert_that!(extract(source)).is_equal_to(Some("# metadata".to_string()));
    }

    #[test]
    fn bom_inside_comment_text_is_preserved() {
        assert_that!(extract("\u{feff}# \u{feff}metadata\u{feff}\nquery Q { x }"))
            .is_equal_to(Some("# \u{feff}metadata\u{feff}".to_string()));
    }

    #[rstest]
    #[case::comma_before_comment(", # detached\nquery Q { x }")]
    #[case::comma_line("# detached\n,\nquery Q { x }")]
    #[case::bom_only_line("# detached\n\u{feff}\nquery Q { x }")]
    fn non_comment_lines_remain_boundaries(#[case] source: &str) {
        assert_that!(extract(source)).is_none();
    }

    #[rstest]
    #[case::block_string(
        "query A { f(arg: \"\"\"\n# string content\"\"\") }\nquery B { b }",
        None
    )]
    #[case::block_string_and_comment(
        "query A { f(arg: \"\"\"\n# string content\"\"\") }\n# B metadata\nquery B { b }",
        Some("# B metadata")
    )]
    #[case::trailing_comment("query A { a } # trailing\nquery B { b }", None)]
    fn previous_definition_bounds_the_comment_scan(
        #[case] source: &str,
        #[case] expected: Option<&str>,
    ) {
        let offset = source.rfind("query ").unwrap();
        let previous_definition_end = source.find('}').unwrap() + 1;
        assert_that!(extract_leading_comments(
            source,
            offset,
            previous_definition_end
        ))
        .is_equal_to(expected.map(str::to_string));
    }
}
