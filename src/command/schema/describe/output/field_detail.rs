use comfy_table::{Table, presets};
use itertools::Itertools;
use rover_schema::{
    describe::type_detail::{ExpandedType, FieldDetail, TypeKind},
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
            ret.name, ret.kind, table
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
                expanded_type_table(exp).map(|t| format!("{} ({})\n{}", exp.name, exp.kind, t))
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

fn expanded_type_table(exp: &ExpandedType) -> Option<Table> {
    match exp.kind {
        TypeKind::Object | TypeKind::Interface | TypeKind::Input => {
            if exp.fields.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Field", "Type"]);
            if !exp.implements.is_empty() {
                let implements = exp
                    .implements
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                table.add_row(["[implements]", &implements]);
            }
            for field in &exp.fields {
                table.add_row([field.name.as_str(), field.return_type.as_str()]);
            }
            Some(table)
        }
        TypeKind::Enum => {
            if exp.enum_values.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Value"]);
            for val in &exp.enum_values {
                table.add_row([val.name.as_str()]);
            }
            Some(table)
        }
        TypeKind::Union => {
            if exp.union_members.is_empty() {
                return None;
            }
            let mut table = Table::new();
            table.load_preset(presets::ASCII_FULL);
            table.set_header(["Member"]);
            for member in &exp.union_members {
                table.add_row([member.as_str()]);
            }
            Some(table)
        }
        TypeKind::Scalar => None,
    }
}

fn via_section(via: &[RootPath]) -> Option<String> {
    if via.is_empty() {
        return None;
    }
    let paths = via.iter().map(|p| p.format_via()).join(", ");
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
        ParsedSchema::parse(sdl)
    }

    fn display(schema: &ParsedSchema, coord: &str) -> String {
        let coord: SchemaCoordinate = coord.parse().unwrap();
        let detail = schema.field_detail(&coord).unwrap();
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
