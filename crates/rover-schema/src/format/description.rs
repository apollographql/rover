use crate::describe::{
    DescribeResult, ExpandedType, FieldDetail, FieldInfo, SchemaOverview, TypeDetail, TypeKind,
};
#[cfg(feature = "search")]
use crate::search::SearchResult;

const MAX_WIDTH: usize = 100;

/// Format a DescribeResult as human-readable description text.
pub fn format_describe(result: &DescribeResult) -> String {
    match result {
        DescribeResult::Overview(overview) => format_overview(overview),
        DescribeResult::TypeDetail(detail) => format_type_detail(detail),
        DescribeResult::FieldDetail(detail) => format_field_detail(detail),
    }
}

fn format_overview(ov: &SchemaOverview) -> String {
    let mut out = String::new();

    // Header line
    out.push_str(&format!(
        "SCHEMA {} [{} types, {} fields",
        ov.graph_ref, ov.total_types, ov.total_fields
    ));
    if ov.total_deprecated > 0 {
        out.push_str(&format!(", {} deprecated", ov.total_deprecated));
    }
    out.push_str("]\n");

    // Query fields
    if !ov.query_fields.is_empty() {
        let names: Vec<&str> = ov.query_fields.iter().map(|f| f.name.as_str()).collect();
        let preview = truncate_list(&names, 6);
        out.push_str(&format!(
            "  Query      \u{203a} {} fields ({})\n",
            ov.query_fields.len(),
            preview
        ));
    }

    // Mutation fields
    if !ov.mutation_fields.is_empty() {
        let names: Vec<&str> = ov.mutation_fields.iter().map(|f| f.name.as_str()).collect();
        let preview = truncate_list(&names, 6);
        out.push_str(&format!(
            "  Mutation   \u{203a} {} fields ({})\n",
            ov.mutation_fields.len(),
            preview
        ));
    }

    out.push('\n');

    // Type categories
    if !ov.objects.is_empty() {
        let preview = truncate_list_owned(&ov.objects, 8);
        out.push_str(&format!(
            "  objects    ({:>2})  {}\n",
            ov.objects.len(),
            preview
        ));
    }
    if !ov.inputs.is_empty() {
        let preview = truncate_list_owned(&ov.inputs, 8);
        out.push_str(&format!(
            "  inputs     ({:>2})  {}\n",
            ov.inputs.len(),
            preview
        ));
    }
    if !ov.enums.is_empty() {
        let preview = truncate_list_owned(&ov.enums, 8);
        out.push_str(&format!(
            "  enums      ({:>2})  {}\n",
            ov.enums.len(),
            preview
        ));
    }
    if !ov.interfaces.is_empty() {
        let preview = truncate_list_owned(&ov.interfaces, 8);
        out.push_str(&format!(
            "  interfaces ({:>2})  {}\n",
            ov.interfaces.len(),
            preview
        ));
    }
    if !ov.unions.is_empty() {
        let preview = truncate_list_owned(&ov.unions, 8);
        out.push_str(&format!(
            "  unions     ({:>2})  {}\n",
            ov.unions.len(),
            preview
        ));
    }
    if !ov.scalars.is_empty() {
        let preview = truncate_list_owned(&ov.scalars, 8);
        out.push_str(&format!(
            "  scalars    ({:>2})  {}\n",
            ov.scalars.len(),
            preview
        ));
    }

    out.trim_end().to_string()
}

fn format_type_detail(detail: &TypeDetail) -> String {
    let mut out = String::new();

    // Header
    let kind_label = match detail.kind {
        TypeKind::Object => "TYPE",
        TypeKind::Interface => "INTERFACE",
        TypeKind::Input => "INPUT",
        TypeKind::Enum => "ENUM",
        TypeKind::Union => "UNION",
        TypeKind::Scalar => "SCALAR",
    };

    out.push_str(&format!("{} {}", kind_label, detail.name));

    match detail.kind {
        TypeKind::Enum => {
            let value_count = detail.enum_values.len()
                + if detail.deprecated_count > 0 {
                    detail.deprecated_count
                } else {
                    0
                };
            out.push_str(&format!(" [{} values", value_count));
        }
        TypeKind::Union => {
            out.push_str(&format!(" [{} members", detail.union_members.len()));
        }
        _ => {
            out.push_str(&format!(" [{} fields", detail.field_count));
        }
    }

    if detail.deprecated_count > 0 {
        out.push_str(&format!(", {} deprecated", detail.deprecated_count));
    }
    out.push(']');

    if !detail.implements.is_empty() {
        out.push_str(&format!(" implements {}", detail.implements.join(" & ")));
    }

    out.push('\n');

    // Description
    if let Some(desc) = &detail.description {
        out.push_str(&format!("  \u{203a} {}\n", desc));

        // Blank line after description before fields/values
        if !detail.fields.is_empty()
            || !detail.enum_values.is_empty()
            || !detail.union_members.is_empty()
        {
            out.push('\n');
        }
    }

    // Fields
    write_field_items(&mut out, &detail.fields, 2);

    // Enum values
    if !detail.enum_values.is_empty() {
        let cols: Vec<String> = detail.enum_values.iter().map(|v| v.name.clone()).collect();
        let col_width = compute_col_width(&cols, 2);
        for (i, val) in detail.enum_values.iter().enumerate() {
            write_item_line(
                &mut out,
                &cols[i],
                val.description.as_deref(),
                val.is_deprecated,
                val.deprecation_reason.as_deref(),
                2,
                col_width,
            );
        }
    }

    // Union members
    if !detail.union_members.is_empty() {
        if detail.kind == TypeKind::Interface {
            // For interfaces, show implementing types
            out.push_str(&format!(
                "  \u{203a} implemented by {}\n",
                detail.union_members.join(", ")
            ));
        } else {
            out.push_str(&format!("  = {}\n", detail.union_members.join(" | ")));
        }
    }

    // Hint when deprecated fields/values are hidden
    if detail.deprecated_count > 0 {
        let showing_deprecated = match detail.kind {
            TypeKind::Enum => detail.enum_values.iter().any(|v| v.is_deprecated),
            _ => detail.fields.iter().any(|f| f.is_deprecated),
        };
        if !showing_deprecated {
            out.push_str(&format!(
                "\n  Use --include-deprecated to show {} hidden deprecated {}.\n",
                detail.deprecated_count,
                if detail.kind == TypeKind::Enum {
                    "values"
                } else {
                    "fields"
                }
            ));
        }
    }

    // Expanded types (--depth)
    for expanded in &detail.expanded_types {
        out.push('\n');
        format_expanded_type(&mut out, expanded);
    }

    out.trim_end().to_string()
}

fn format_field_detail(detail: &FieldDetail) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "FIELD {}.{} \u{2192} {}",
        detail.type_name, detail.field_name, detail.return_type
    ));
    if detail.arg_count > 0 {
        out.push_str(&format!(
            " [{} arg{}]",
            detail.arg_count,
            if detail.arg_count == 1 { "" } else { "s" }
        ));
    }
    out.push('\n');

    // Description
    if let Some(desc) = &detail.description {
        out.push_str(&format!("  \u{203a} {}\n", desc));
    }

    // Via paths
    for via in &detail.via {
        out.push_str(&format!("  via {}\n", via));
    }

    // Deprecation
    if detail.is_deprecated {
        if let Some(reason) = &detail.deprecation_reason {
            out.push_str(&format!("  \u{26a0}\u{fe0f} DEPRECATED: {}\n", reason));
        } else {
            out.push_str("  \u{26a0}\u{fe0f} DEPRECATED\n");
        }
    }

    // Args
    if !detail.args.is_empty() {
        out.push_str("\n  args:\n");
        let arg_cols: Vec<String> = detail
            .args
            .iter()
            .map(|a| format!("{}: {}", a.name, a.arg_type))
            .collect();
        let col_width = compute_col_width(&arg_cols, 4);
        for (i, arg) in detail.args.iter().enumerate() {
            write_item_line(
                &mut out,
                &arg_cols[i],
                arg.description.as_deref(),
                false,
                None,
                4,
                col_width,
            );
        }
    }

    // Input type expansions
    for input in &detail.input_expansions {
        out.push_str(&format!("\n  input {}:\n", input.name));
        write_field_items(&mut out, &input.fields, 4);
    }

    // Return type expansion
    if let Some(ret) = &detail.return_expansion {
        out.push_str(&format!("\n  returns {}:\n", ret.name));
        write_field_items(&mut out, &ret.fields, 4);
    }

    out.trim_end().to_string()
}

/// Format a search result in description format.
#[cfg(feature = "search")]
pub fn format_search(results: &[SearchResult], query: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "SEARCH \"{}\" [{} path{}]\n",
        query,
        results.len(),
        if results.len() == 1 { "" } else { "s" }
    ));

    for result in results {
        out.push_str(&format!(
            "\n\u{2500}\u{2500} {} \u{2500}\u{2500}\n\n",
            result.path_header
        ));

        for expanded in &result.types {
            out.push_str(&format!("  {}", expanded.name));
            if !expanded.union_members.is_empty() && expanded.kind == TypeKind::Interface {
                out.push_str(&format!(
                    " (interface \u{2192} {})",
                    expanded.union_members.join(", ")
                ));
            }
            if !expanded.fields.is_empty() {
                out.push_str(&format!(" [{} fields]", expanded.fields.len()));
            }
            out.push('\n');

            write_field_items(&mut out, &expanded.fields, 4);
            out.push('\n');
        }
    }

    out.trim_end().to_string()
}

// Helpers

/// Compute column width for name columns based on the widest item.
/// Ensures at least a 2-char gap before the › separator.
fn compute_col_width(items: &[String], indent: usize) -> usize {
    let longest = items.iter().map(|s| s.len()).max().unwrap_or(0);
    let col = longest + 2; // min 2-char gap before ›
    let max = (MAX_WIDTH - indent) / 2;
    col.clamp(16, max)
}

/// Word-wrap description text to fit within `avail` chars per line.
/// Continuation lines are indented to `cont_indent` spaces.
/// Embedded newlines are reflowed (replaced with spaces) before wrapping.
fn wrap_description(text: &str, avail: usize, cont_indent: usize) -> String {
    // Normalize embedded newlines into spaces so we can re-wrap cleanly.
    // Collapse any \r\n or \n followed by whitespace into a single space.
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' {
            // Skip any following whitespace (including more newlines)
            while chars.peek().is_some_and(|&ch| ch.is_whitespace()) {
                chars.next();
            }
            // Replace with a single space (unless we're at the start or end)
            if !normalized.is_empty() && chars.peek().is_some() {
                normalized.push(' ');
            }
        } else {
            normalized.push(c);
        }
    }

    if avail == 0 || normalized.len() <= avail {
        return normalized;
    }

    let mut result = String::new();
    let mut remaining = normalized.as_str();
    let mut first = true;

    while !remaining.is_empty() {
        if !first {
            result.push('\n');
            for _ in 0..cont_indent {
                result.push(' ');
            }
        }

        if remaining.len() <= avail {
            result.push_str(remaining);
            break;
        }

        let break_at = remaining[..avail].rfind(' ').unwrap_or(avail);
        let after_break = remaining[break_at..].trim_start();
        // Don't widow short trailing fragments (e.g. a lone ".")
        if after_break.len() <= 2 {
            result.push_str(remaining);
            break;
        }
        result.push_str(&remaining[..break_at]);
        remaining = after_break;
        first = false;
    }

    result
}

/// Write one field/enum-value line with consistent formatting.
///
/// For 2-space indent, deprecated fields use `⚠` as a gutter marker replacing the first space.
/// For 4-space indent, deprecated fields use `  ⚠ ` (⚠ replaces one inner space).
/// If deprecated with a reason, appends a `↳ Deprecated: reason` line below.
fn write_item_line(
    out: &mut String,
    name_col: &str,
    desc: Option<&str>,
    is_deprecated: bool,
    dep_reason: Option<&str>,
    indent: usize,
    col_width: usize,
) {
    // Write indent with optional deprecation gutter marker
    if is_deprecated {
        if indent <= 2 {
            out.push('\u{26a0}');
            for _ in 1..indent {
                out.push(' ');
            }
        } else {
            for _ in 0..indent.saturating_sub(2) {
                out.push(' ');
            }
            out.push('\u{26a0}');
            out.push(' ');
        }
    } else {
        for _ in 0..indent {
            out.push(' ');
        }
    }

    // Write name column, padded to col_width with minimum 2-char gap
    out.push_str(name_col);

    let name_len = name_col.len();
    let pad = if name_len + 2 <= col_width {
        col_width - name_len
    } else {
        2 // always at least 2 spaces before ›
    };

    if let Some(desc) = desc {
        for _ in 0..pad {
            out.push(' ');
        }
        out.push_str("\u{203a} ");

        let desc_start = indent + name_len + pad + 2; // +2 for "› "
        let avail = MAX_WIDTH.saturating_sub(desc_start);
        let wrapped = wrap_description(desc, avail, desc_start);
        out.push_str(&wrapped);
    }
    out.push('\n');

    // Deprecated reason on next line, visually attached with ↳
    if is_deprecated && let Some(reason) = dep_reason {
        for _ in 0..indent + 2 {
            out.push(' ');
        }
        out.push_str("\u{21b3} Deprecated: ");
        out.push_str(reason);
        out.push('\n');
    }
}

/// Write a list of FieldInfo items with computed column alignment.
fn write_field_items(out: &mut String, fields: &[FieldInfo], indent: usize) {
    if fields.is_empty() {
        return;
    }
    let cols: Vec<String> = fields
        .iter()
        .map(|f| format!("{}: {}", f.name, f.return_type))
        .collect();
    let col_width = compute_col_width(&cols, indent);
    for (i, field) in fields.iter().enumerate() {
        write_item_line(
            out,
            &cols[i],
            field.description.as_deref(),
            field.is_deprecated,
            field.deprecation_reason.as_deref(),
            indent,
            col_width,
        );
    }
}

fn format_expanded_type(out: &mut String, expanded: &ExpandedType) {
    out.push_str(&format!("  \u{2508} {}", expanded.name));
    if expanded.kind == TypeKind::Interface && !expanded.union_members.is_empty() {
        out.push_str(&format!(
            " (interface \u{2192} {})",
            expanded.union_members.join(", ")
        ));
    }
    if !expanded.fields.is_empty() {
        out.push_str(&format!(
            " [{} field{}]",
            expanded.fields.len(),
            if expanded.fields.len() == 1 { "" } else { "s" }
        ));
    }
    out.push('\n');

    write_field_items(out, &expanded.fields, 4);
}

fn truncate_list(items: &[&str], max: usize) -> String {
    if items.len() <= max {
        items.join(", ")
    } else {
        format!("{}, ...", items[..max].join(", "))
    }
}

fn truncate_list_owned(items: &[String], max: usize) -> String {
    if items.len() <= max {
        items.join(" ")
    } else {
        format!("{} ...", items[..max].join(" "))
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::Schema;

    use super::*;
    use crate::describe;

    fn test_schema() -> Schema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        match Schema::parse(sdl, "test.graphql") {
            Ok(s) => s,
            Err(e) => e.partial,
        }
    }

    #[test]
    fn deprecated_field_gutter_marker_with_reason() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "Post", true, 0).unwrap();
        let output = format_type_detail(&detail);
        // ⚠ gutter marker on the field line
        assert!(output.contains("\u{26a0}"));
        // Reason on the next line
        assert!(output.contains("\u{21b3} Deprecated: Use slug instead"));
    }

    #[test]
    fn deprecated_field_gutter_marker_without_reason() {
        let mut out = String::new();
        write_item_line(&mut out, "oldField: String", None, true, None, 2, 20);
        assert!(
            out.starts_with("\u{26a0}"),
            "should start with ⚠ gutter marker"
        );
        assert!(
            !out.contains("Deprecated:"),
            "no reason line when reason is None"
        );
    }

    #[test]
    fn deprecated_enum_value_marker() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "SortOrder", true, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("RELEVANCE"));
        assert!(output.contains("\u{26a0}"));
        assert!(output.contains("\u{21b3} Deprecated: Use TOP instead"));
    }

    #[test]
    fn hidden_deprecated_hint_for_fields() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "Post", false, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("Use --include-deprecated to show"));
        assert!(output.contains("hidden deprecated fields"));
        assert!(!output.contains("oldSlug"));
    }

    #[test]
    fn hidden_deprecated_hint_for_enum_values() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "SortOrder", false, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("Use --include-deprecated to show"));
        assert!(output.contains("hidden deprecated values"));
        assert!(!output.contains("RELEVANCE"));
    }

    #[test]
    fn no_hint_when_deprecated_included() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "Post", true, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(!output.contains("Use --include-deprecated"));
        assert!(output.contains("oldSlug"));
    }

    #[test]
    fn no_hint_when_no_deprecated() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "Category", true, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(!output.contains("Use --include-deprecated"));
    }

    #[test]
    fn expanded_type_deprecated_field_marker() {
        let schema = test_schema();
        // User has deprecated legacyId; expand via Post --depth 1
        let detail = describe::type_detail(&schema, "Post", true, 1).unwrap();
        let output = format_type_detail(&detail);
        // The expanded User type should show the deprecated marker
        assert!(output.contains("legacyId"));
        assert!(output.contains("\u{21b3} Deprecated: Use id instead"));
    }

    #[test]
    fn column_alignment_consistent() {
        let schema = test_schema();
        let detail = describe::type_detail(&schema, "Post", false, 0).unwrap();
        let output = format_type_detail(&detail);
        // Field lines (containing "name: Type") with descriptions should have
        // at least a 2-space gap before the › separator
        for line in output.lines() {
            let trimmed = line.trim_start();
            // Only check field lines: must have ":" before "›" (name: Type pattern)
            if let (Some(colon_pos), Some(sep_pos)) = (trimmed.find(':'), trimmed.find('\u{203a}'))
                && colon_pos < sep_pos
            {
                let abs_sep = line.find('\u{203a}').unwrap();
                let before_sep = &line[..abs_sep];
                assert!(
                    before_sep.ends_with("  "),
                    "should have at least 2-space gap before ›: {:?}",
                    line
                );
            }
        }
    }

    #[test]
    fn description_wrapping() {
        let mut out = String::new();
        let long_desc = "This is a very long description that should wrap to multiple lines because it exceeds the available width when combined with the field name column";
        write_item_line(&mut out, "field: Type", Some(long_desc), false, None, 2, 20);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.len() > 1, "long description should wrap");
        // Continuation lines should be indented
        for line in &lines[1..] {
            assert!(
                line.starts_with("                        "),
                "continuation should be indented: {:?}",
                line
            );
        }
    }

    #[test]
    fn compute_col_width_basics() {
        let items = vec!["name: String!".to_string(), "id: ID!".to_string()];
        let width = compute_col_width(&items, 2);
        // longest is 13 ("name: String!"), so width = 15, clamped min to 16
        assert_eq!(width, 16);

        let long_items = vec!["veryLongFieldName: [SomeComplexType!]!".to_string()];
        let width = compute_col_width(&long_items, 2);
        // longest is 38, so width = 40, within clamp range
        assert_eq!(width, 40);
    }

    #[test]
    fn wrap_description_short_text() {
        let result = wrap_description("short text", 50, 10);
        assert_eq!(result, "short text");
    }

    #[test]
    fn wrap_description_long_text() {
        let text = "This is a longer description that needs wrapping";
        let result = wrap_description(text, 25, 10);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.len() > 1);
        // First line fits within avail
        assert!(lines[0].len() <= 25);
        // Continuation lines are indented
        for line in &lines[1..] {
            assert!(line.starts_with("          ")); // 10 spaces
        }
    }

    #[test]
    fn wrap_description_reflows_embedded_newlines() {
        let text = "First line of text.\nSecond line\nThird line that keeps going.";
        let result = wrap_description(text, 50, 10);
        // Embedded newlines should be collapsed into spaces and re-wrapped
        assert!(
            !result.contains("Second line\n"),
            "embedded newline should be reflowed: {:?}",
            result
        );
        // The text should still be present, just reflowed
        assert!(result.contains("First line of text."));
        assert!(result.contains("Second line"));
        assert!(result.contains("Third line"));
    }

    #[test]
    fn deprecated_gutter_indent_2() {
        let mut out = String::new();
        write_item_line(&mut out, "field: Type", None, true, None, 2, 16);
        // Should start with ⚠ (replacing first indent space) + 1 space
        assert!(out.starts_with("\u{26a0} field: Type"));
    }

    #[test]
    fn deprecated_gutter_indent_4() {
        let mut out = String::new();
        write_item_line(
            &mut out,
            "field: Type",
            None,
            true,
            Some("old field"),
            4,
            16,
        );
        // Should start with 2 spaces + ⚠ + space
        assert!(out.starts_with("  \u{26a0} field: Type"));
        assert!(out.contains("\u{21b3} Deprecated: old field"));
    }
}
