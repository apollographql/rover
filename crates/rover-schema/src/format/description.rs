use apollo_compiler::Name;

use super::{ARROW, DEPRECATED_MARKER, DOTTED, HOOK_ARROW, SEPARATOR};
use crate::describe::{
    DescribeResult, EnumDetail, ExpandedType, FieldDetail, FieldInfo, InputDetail,
    InterfaceDetail, ObjectDetail, ScalarDetail, SchemaOverview, TypeDetail, TypeKind, UnionDetail,
};

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
        ov.schema_source, ov.total_types, ov.total_fields
    ));
    if ov.total_deprecated > 0 {
        out.push_str(&format!(", {} deprecated", ov.total_deprecated));
    }
    out.push_str("]\n");

    // Query fields
    if !ov.query_fields.is_empty() {
        let names: Vec<&str> = ov.query_fields.iter().map(|f| f.name().as_str()).collect();
        let preview = truncate_list(&names, 6);
        out.push_str(&format!(
            "  Query      {SEPARATOR} {} fields ({})\n",
            ov.query_fields.len(),
            preview
        ));
    }

    // Mutation fields
    if !ov.mutation_fields.is_empty() {
        let names: Vec<&str> = ov
            .mutation_fields
            .iter()
            .map(|f| f.name().as_str())
            .collect();
        let preview = truncate_list(&names, 6);
        out.push_str(&format!(
            "  Mutation   {SEPARATOR} {} fields ({})\n",
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
    match detail {
        TypeDetail::Object(obj) => format_object_detail(obj),
        TypeDetail::Interface(iface) => format_interface_detail(iface),
        TypeDetail::Input(inp) => format_input_detail(inp),
        TypeDetail::Enum(e) => format_enum_detail(e),
        TypeDetail::Union(u) => format_union_detail(u),
        TypeDetail::Scalar(s) => format_scalar_detail(s),
    }
}

fn format_object_detail(obj: &ObjectDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("TYPE {}", obj.name));
    out.push_str(&format!(" [{} fields", obj.fields.field_count()));
    if obj.fields.deprecated_count > 0 {
        out.push_str(&format!(", {} deprecated", obj.fields.deprecated_count));
    }
    out.push(']');
    if !obj.implements.is_empty() {
        out.push_str(&format!(" implements {}", obj.implements.join(" & ")));
    }
    out.push('\n');
    if let Some(desc) = &obj.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
        if !obj.fields.fields().is_empty() {
            out.push('\n');
        }
    }
    write_field_items(&mut out, obj.fields.fields(), 2);
    if obj.fields.deprecated_count > 0 && !obj.fields.fields().iter().any(|f| f.is_deprecated) {
        out.push_str(&format!(
            "\n  Use --include-deprecated to show {} hidden deprecated fields.\n",
            obj.fields.deprecated_count
        ));
    }
    for expanded in &obj.fields.expanded_types {
        out.push('\n');
        format_expanded_type(&mut out, expanded);
    }
    out.trim_end().to_string()
}

fn format_interface_detail(iface: &InterfaceDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("INTERFACE {}", iface.name));
    out.push_str(&format!(" [{} fields", iface.fields.field_count()));
    if iface.fields.deprecated_count > 0 {
        out.push_str(&format!(", {} deprecated", iface.fields.deprecated_count));
    }
    out.push(']');
    if !iface.implements.is_empty() {
        out.push_str(&format!(" implements {}", iface.implements.join(" & ")));
    }
    out.push('\n');
    if let Some(desc) = &iface.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
        if !iface.fields.fields().is_empty() || !iface.implementors.is_empty() {
            out.push('\n');
        }
    }
    write_field_items(&mut out, iface.fields.fields(), 2);
    if !iface.implementors.is_empty() {
        out.push_str(&format!(
            "  {SEPARATOR} implemented by {}\n",
            iface.implementors.join(", ")
        ));
    }
    if iface.fields.deprecated_count > 0 && !iface.fields.fields().iter().any(|f| f.is_deprecated) {
        out.push_str(&format!(
            "\n  Use --include-deprecated to show {} hidden deprecated fields.\n",
            iface.fields.deprecated_count
        ));
    }
    for expanded in &iface.fields.expanded_types {
        out.push('\n');
        format_expanded_type(&mut out, expanded);
    }
    out.trim_end().to_string()
}

fn format_input_detail(inp: &InputDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("INPUT {} [{} fields]\n", inp.name, inp.fields.field_count));
    if let Some(desc) = &inp.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
        if !inp.fields.fields().is_empty() {
            out.push('\n');
        }
    }
    write_field_items(&mut out, inp.fields.fields(), 2);
    out.trim_end().to_string()
}

fn format_enum_detail(e: &EnumDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("ENUM {}", e.name));
    out.push_str(&format!(" [{} values", e.value_count));
    if e.deprecated_count > 0 {
        out.push_str(&format!(", {} deprecated", e.deprecated_count));
    }
    out.push_str("]\n");
    if let Some(desc) = &e.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
        if !e.values.is_empty() {
            out.push('\n');
        }
    }
    if !e.values.is_empty() {
        let cols: Vec<String> = e.values.iter().map(|v| v.name.to_string()).collect();
        let col_width = compute_col_width(&cols, 2);
        for (i, val) in e.values.iter().enumerate() {
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
    if e.deprecated_count > 0 && !e.values.iter().any(|v| v.is_deprecated) {
        out.push_str(&format!(
            "\n  Use --include-deprecated to show {} hidden deprecated values.\n",
            e.deprecated_count
        ));
    }
    out.trim_end().to_string()
}

fn format_union_detail(u: &UnionDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("UNION {} [{} members]\n", u.name, u.members.len()));
    if let Some(desc) = &u.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
        if !u.members.is_empty() {
            out.push('\n');
        }
    }
    if !u.members.is_empty() {
        out.push_str(&format!("  = {}\n", u.members.join(" | ")));
    }
    out.trim_end().to_string()
}

fn format_scalar_detail(s: &ScalarDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("SCALAR {}\n", s.name));
    if let Some(desc) = &s.description {
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
    }
    out.trim_end().to_string()
}

fn format_field_detail(detail: &FieldDetail) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "FIELD {}.{} {ARROW} {}",
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
        out.push_str(&format!("  {SEPARATOR} {}\n", desc));
    }

    // Via paths
    for via in &detail.via {
        out.push_str(&format!("  via {}\n", via.format_via()));
    }

    // Deprecation
    if detail.is_deprecated {
        if let Some(reason) = &detail.deprecation_reason {
            out.push_str(&format!("  {DEPRECATED_MARKER} DEPRECATED: {}\n", reason));
        } else {
            out.push_str(&format!("  {DEPRECATED_MARKER} DEPRECATED\n"));
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
            out.push(DEPRECATED_MARKER);
            for _ in 1..indent {
                out.push(' ');
            }
        } else {
            for _ in 0..indent.saturating_sub(2) {
                out.push(' ');
            }
            out.push(DEPRECATED_MARKER);
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
        out.push(SEPARATOR);
        out.push(' ');

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
        out.push(HOOK_ARROW);
        out.push_str(" Deprecated: ");
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
    out.push_str(&format!("  {DOTTED} {}", expanded.name));
    if expanded.kind == TypeKind::Interface && !expanded.union_members.is_empty() {
        out.push_str(&format!(
            " (interface {ARROW} {})",
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

fn truncate_list_owned(items: &[Name], max: usize) -> String {
    if items.len() <= max {
        items.join(" ")
    } else {
        format!("{} ...", items[..max].join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParsedSchema;

    fn test_schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[test]
    fn deprecated_field_gutter_marker_with_reason() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("Post").unwrap(), true, 0).unwrap();
        let output = format_type_detail(&detail);
        // ⚠ gutter marker on the field line
        assert!(output.contains(DEPRECATED_MARKER));
        // Reason on the next line
        assert!(output.contains(&format!("{HOOK_ARROW} Deprecated: Use slug instead")));
    }

    #[test]
    fn deprecated_field_gutter_marker_without_reason() {
        let mut out = String::new();
        write_item_line(&mut out, "oldField: String", None, true, None, 2, 20);
        assert!(
            out.starts_with(DEPRECATED_MARKER),
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
        let detail = schema.type_detail(&Name::new("SortOrder").unwrap(), true, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("RELEVANCE"));
        assert!(output.contains(DEPRECATED_MARKER));
        assert!(output.contains(&format!("{HOOK_ARROW} Deprecated: Use TOP instead")));
    }

    #[test]
    fn hidden_deprecated_hint_for_fields() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("Post").unwrap(), false, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("Use --include-deprecated to show"));
        assert!(output.contains("hidden deprecated fields"));
        assert!(!output.contains("oldSlug"));
    }

    #[test]
    fn hidden_deprecated_hint_for_enum_values() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("SortOrder").unwrap(), false, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(output.contains("Use --include-deprecated to show"));
        assert!(output.contains("hidden deprecated values"));
        assert!(!output.contains("RELEVANCE"));
    }

    #[test]
    fn no_hint_when_deprecated_included() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("Post").unwrap(), true, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(!output.contains("Use --include-deprecated"));
        assert!(output.contains("oldSlug"));
    }

    #[test]
    fn no_hint_when_no_deprecated() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("Comment").unwrap(), false, 0).unwrap();
        let output = format_type_detail(&detail);
        assert!(!output.contains("Use --include-deprecated"));
    }

    #[test]
    fn expanded_type_deprecated_field_marker() {
        let schema = test_schema();
        // User has deprecated legacyId; expand via Post --depth 1
        let detail = schema.type_detail(&Name::new("Post").unwrap(), true, 1).unwrap();
        let output = format_type_detail(&detail);
        // The expanded User type should show the deprecated marker
        assert!(output.contains("legacyId"));
        assert!(output.contains(&format!("{HOOK_ARROW} Deprecated: Use id instead")));
    }

    #[test]
    fn column_alignment_consistent() {
        let schema = test_schema();
        let detail = schema.type_detail(&Name::new("Post").unwrap(), false, 0).unwrap();
        let output = format_type_detail(&detail);
        // Field lines (containing "name: Type") with descriptions should have
        // at least a 2-space gap before the › separator
        for line in output.lines() {
            let trimmed = line.trim_start();
            // Only check field lines: must have ":" before "›" (name: Type pattern)
            if let (Some(colon_pos), Some(sep_pos)) = (trimmed.find(':'), trimmed.find(SEPARATOR))
                && colon_pos < sep_pos
            {
                let abs_sep = line.find(SEPARATOR).unwrap();
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
        assert!(out.starts_with(&format!("{DEPRECATED_MARKER} field: Type")));
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
        assert!(out.starts_with(&format!("  {DEPRECATED_MARKER} field: Type")));
        assert!(out.contains(&format!("{HOOK_ARROW} Deprecated: old field")));
    }
}
