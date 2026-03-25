use comfy_table::{Table, presets};
use itertools::Itertools;
use rover_schema::{
    describe::type_detail::{ExpandedType, FieldDetail},
    root_paths::RootPath,
};

pub struct FieldDetailDisplay<'a> {
    detail: &'a FieldDetail,
}

impl<'a> FieldDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [
            Some(self.header()),
            self.deprecated(),
            self.description(),
            self.args(),
            self.via(),
            self.return_type(),
            self.input_types(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        format!("FIELD {}.{}: {}", d.type_name, d.field_name, d.return_type)
    }

    fn deprecated(&self) -> Option<String> {
        if !self.detail.is_deprecated {
            return None;
        }
        Some(match &self.detail.deprecation_reason {
            Some(reason) => format!("DEPRECATED: {}", reason),
            None => "DEPRECATED".to_string(),
        })
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn args(&self) -> Option<String> {
        if self.detail.args.is_empty() {
            return None;
        }

        let mut table = Table::new();
        table.load_preset(presets::ASCII_FULL);
        table.set_header(["Arg", "Type", "Notes"]);

        for arg in &self.detail.args {
            let notes = match (&arg.description, &arg.default_value) {
                (Some(desc), Some(default)) => format!("{} (default: {})", desc, default),
                (Some(desc), None) => desc.clone(),
                (None, Some(default)) => format!("default: {}", default),
                (None, None) => String::new(),
            };
            table.add_row([arg.name.as_str(), arg.arg_type.as_str(), &notes]);
        }

        Some(format!("{} args\nArgs\n{table}", self.detail.arg_count))
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }

    fn return_type(&self) -> Option<String> {
        let ret = self.detail.return_expansion.as_ref()?;
        let table = expanded_type_table(ret)?;
        Some(format!(
            "Return type: {} ({})\n{}",
            ret.name(),
            expanded_type_kind(ret),
            table
        ))
    }

    fn input_types(&self) -> Option<String> {
        if self.detail.input_expansions.is_empty() {
            return None;
        }

        let sections: Vec<String> = self
            .detail
            .input_expansions
            .iter()
            .filter_map(|exp| {
                expanded_type_table(exp)
                    .map(|t| format!("{} ({})\n{}", exp.name(), expanded_type_kind(exp), t))
            })
            .collect();

        if sections.is_empty() {
            return None;
        }

        Some(format!("Input types\n{}", sections.join("\n\n")))
    }
}

impl<'a> From<&'a FieldDetail> for FieldDetailDisplay<'a> {
    fn from(detail: &'a FieldDetail) -> Self {
        FieldDetailDisplay { detail }
    }
}

fn expanded_type_kind(exp: &ExpandedType) -> &'static str {
    match exp {
        ExpandedType::Object { .. } => "object",
        ExpandedType::Interface { .. } => "interface",
        ExpandedType::Input { .. } => "input",
        ExpandedType::Enum { .. } => "enum",
        ExpandedType::Union { .. } => "union",
    }
}

fn expanded_type_table(exp: &ExpandedType) -> Option<Table> {
    match exp {
        ExpandedType::Object {
            fields, implements, ..
        }
        | ExpandedType::Interface {
            fields, implements, ..
        } => {
            if fields.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Field", "Type"]);
            if !implements.is_empty() {
                let impl_str = implements
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                table.add_row(["[implements]", &impl_str]);
            }
            for field in fields {
                table.add_row([field.name.as_str(), field.return_type.as_str()]);
            }
            Some(table)
        }
        ExpandedType::Input { fields, .. } => {
            if fields.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Field", "Type"]);
            for field in fields {
                table.add_row([field.name.as_str(), field.field_type.as_str()]);
            }
            Some(table)
        }
        ExpandedType::Enum { values, .. } => {
            if values.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Value"]);
            for val in values {
                table.add_row([val.name.as_str()]);
            }
            Some(table)
        }
        ExpandedType::Union { members, .. } => {
            if members.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Member"]);
            for member in members {
                table.add_row([member.as_str()]);
            }
            Some(table)
        }
    }
}

fn format_root_path(path: &RootPath) -> String {
    let val = serde_json::to_value(path).unwrap_or_default();
    let segs = val["segments"].as_array().cloned().unwrap_or_default();
    segs.iter()
        .map(|s| {
            format!(
                "{}.{}",
                s["type_name"].as_str().unwrap_or("?"),
                s["field_name"].as_str().unwrap_or("?"),
            )
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn via_section(via: &[RootPath]) -> Option<String> {
    if via.is_empty() {
        return None;
    }
    let paths = via.iter().map(format_root_path).join(", ");
    Some(format!("Available via: {}", paths))
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::FieldDetailDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema, coord: &str) -> String {
        let coord: SchemaCoordinate = coord.parse().unwrap();
        let SchemaCoordinate::TypeAttribute(ref tac) = coord else {
            panic!("expected a field coordinate");
        };
        let detail = schema.field_detail(tac).unwrap();
        FieldDetailDisplay::from(&detail).display()
    }

    // --- Header ---

    #[rstest]
    fn header_contains_type_field_and_return_type(schema: ParsedSchema) {
        let out = display(&schema, "Query.post");
        assert_that!(out).starts_with("FIELD Query.post:");
        assert_that!(out).contains("User");
    }

    // --- Deprecated ---

    #[rstest]
    fn deprecated_field_shows_notice_with_reason(schema: ParsedSchema) {
        let out = display(&schema, "Post.oldSlug");
        assert_that!(out).contains("DEPRECATED: Use slug instead");
    }

    #[rstest]
    fn non_deprecated_field_has_no_deprecated_notice(schema: ParsedSchema) {
        let out = display(&schema, "Post.title");
        assert_that!(out).does_not_contain("DEPRECATED");
    }

    // --- Description ---

    #[rstest]
    fn field_description_shown_when_present(schema: ParsedSchema) {
        let out = display(&schema, "Post.body");
        assert_that!(out).contains("The body content");
    }

    // --- Args ---

    #[rstest]
    fn field_with_args_shows_args_section(schema: ParsedSchema) {
        let out = display(&schema, "User.posts");
        assert_that!(out).contains("args");
        assert_that!(out).contains("Args");
        assert_that!(out).contains("limit");
        assert_that!(out).contains("offset");
    }

    #[rstest]
    fn field_without_args_has_no_args_section(schema: ParsedSchema) {
        let out = display(&schema, "Post.title");
        assert_that!(out).does_not_contain("Args");
    }

    #[rstest]
    fn args_table_shows_arg_types(schema: ParsedSchema) {
        let out = display(&schema, "User.posts");
        assert_that!(out).contains("Int");
    }

    // --- Via ---

    #[rstest]
    fn field_includes_via_section(schema: ParsedSchema) {
        let out = display(&schema, "Post.title");
        assert_that!(out).contains("Available via:");
        assert_that!(out).contains("Query.post");
    }

    // --- Return type expansion ---

    #[rstest]
    fn return_type_expansion_shown_for_object_field(schema: ParsedSchema) {
        let out = display(&schema, "Query.post");
        assert_that!(out).contains("Return type:");
        assert_that!(out).contains("User");
    }

    #[rstest]
    fn return_type_expansion_shows_fields(schema: ParsedSchema) {
        let out = display(&schema, "Query.post");
        // Return expansion for Post should include Post's fields
        assert_that!(out).contains("title");
        assert_that!(out).contains("body");
    }

    // --- Input type expansion ---

    #[rstest]
    fn input_type_expansion_shown_for_mutation_with_input_arg(schema: ParsedSchema) {
        let out = display(&schema, "Mutation.createPost");
        assert_that!(out).contains("Input types");
        assert_that!(out).contains("CreatePostInput");
    }

    #[rstest]
    fn input_type_expansion_shows_input_fields(schema: ParsedSchema) {
        let out = display(&schema, "Mutation.createPost");
        assert_that!(out).contains("title");
        assert_that!(out).contains("body");
        assert_that!(out).contains("categoryId");
    }

    #[rstest]
    fn field_without_input_args_has_no_input_types_section(schema: ParsedSchema) {
        let out = display(&schema, "Query.post");
        assert_that!(out).does_not_contain("Input types");
    }
}
