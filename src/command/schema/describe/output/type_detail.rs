use comfy_table::{Table, presets};
use itertools::Itertools;
use rover_schema::{
    describe::type_detail::{
        EnumDetail, ExtendedFieldsDetail, FieldInfo, InputDetail, InputFieldInfo, InterfaceDetail,
        ObjectDetail, ScalarDetail, TypeDetail, UnionDetail,
    },
    root_paths::RootPath,
};

pub struct TypeDetailDisplay<'a> {
    detail: &'a TypeDetail,
}

impl<'a> TypeDetailDisplay<'a> {
    pub fn display(&self) -> String {
        match self.detail {
            TypeDetail::Object(detail) => ObjectDetailDisplay { detail }.display(),
            TypeDetail::Interface(detail) => InterfaceDetailDisplay { detail }.display(),
            TypeDetail::Input(detail) => InputDetailDisplay { detail }.display(),
            TypeDetail::Enum(detail) => EnumDetailDisplay { detail }.display(),
            TypeDetail::Union(detail) => UnionDetailDisplay { detail }.display(),
            TypeDetail::Scalar(detail) => ScalarDetailDisplay { detail }.display(),
        }
    }
}

impl<'a> From<&'a TypeDetail> for TypeDetailDisplay<'a> {
    fn from(detail: &'a TypeDetail) -> Self {
        TypeDetailDisplay { detail }
    }
}

// --- Object ---

struct ObjectDetailDisplay<'a> {
    detail: &'a ObjectDetail,
}

impl<'a> ObjectDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            self.implements(),
            Some(self.summary()),
            Some(self.fields()),
            self.via(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (object)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn implements(&self) -> Option<String> {
        if self.detail.implements.is_empty() {
            return None;
        }
        Some(format!(
            "implements {}",
            self.detail.implements.iter().map(|n| n.as_str()).join(", ")
        ))
    }

    fn summary(&self) -> String {
        fields_summary(&self.detail.fields)
    }

    fn fields(&self) -> String {
        format!("Fields\n{}", fields_table(self.detail.fields.fields()))
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }
}

// --- Interface ---

struct InterfaceDetailDisplay<'a> {
    detail: &'a InterfaceDetail,
}

impl<'a> InterfaceDetailDisplay<'a> {
    fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            self.implements(),
            Some(self.summary()),
            Some(self.fields()),
            self.implementors(),
            self.via(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (interface)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn implements(&self) -> Option<String> {
        if self.detail.implements.is_empty() {
            return None;
        }
        Some(format!(
            "implements {}",
            self.detail.implements.iter().map(|n| n.as_str()).join(", ")
        ))
    }

    fn summary(&self) -> String {
        fields_summary(&self.detail.fields)
    }

    fn fields(&self) -> String {
        format!("Fields\n{}", fields_table(self.detail.fields.fields()))
    }

    fn implementors(&self) -> Option<String> {
        if self.detail.implementors.is_empty() {
            return None;
        }
        Some(format!(
            "Implemented by: {}",
            self.detail
                .implementors
                .iter()
                .map(|n| n.as_str())
                .join(", ")
        ))
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }
}

// --- Input ---

struct InputDetailDisplay<'a> {
    detail: &'a InputDetail,
}

impl<'a> InputDetailDisplay<'a> {
    fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            Some(self.summary()),
            Some(self.fields()),
            self.via(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (input)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn summary(&self) -> String {
        format!("{} fields", self.detail.field_count)
    }

    fn fields(&self) -> String {
        format!("Fields\n{}", input_fields_table(&self.detail.fields))
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }
}

// --- Enum ---

struct EnumDetailDisplay<'a> {
    detail: &'a EnumDetail,
}

impl<'a> EnumDetailDisplay<'a> {
    fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            Some(self.summary()),
            Some(self.values()),
            self.via(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (enum)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn summary(&self) -> String {
        if self.detail.deprecated_count > 0 {
            format!(
                "{} values\n{} deprecated values",
                self.detail.value_count, self.detail.deprecated_count
            )
        } else {
            format!("{} values", self.detail.value_count)
        }
    }

    fn values(&self) -> String {
        let mut table = Table::new();
        table.load_preset(presets::ASCII_FULL);
        table.set_header(["Value", "Description"]);

        for val in &self.detail.values {
            let desc = match (&val.description, val.is_deprecated, &val.deprecation_reason) {
                (_, true, Some(reason)) => format!("(deprecated: {})", reason),
                (_, true, None) => "(deprecated)".to_string(),
                (Some(d), false, _) => d.clone(),
                (None, false, _) => String::new(),
            };
            table.add_row([val.name.as_str(), &desc]);
        }

        format!("Values\n{table}")
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }
}

// --- Union ---

struct UnionDetailDisplay<'a> {
    detail: &'a UnionDetail,
}

impl<'a> UnionDetailDisplay<'a> {
    fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            Some(self.members()),
            self.via(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (union)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn members(&self) -> String {
        format!(
            "Members: {}",
            self.detail.members.iter().map(|n| n.as_str()).join(", ")
        )
    }

    fn via(&self) -> Option<String> {
        via_section(&self.detail.via)
    }
}

// --- Scalar ---

struct ScalarDetailDisplay<'a> {
    detail: &'a ScalarDetail,
}

impl<'a> ScalarDetailDisplay<'a> {
    fn display(&self) -> String {
        [Some(self.header()), self.description()]
            .into_iter()
            .flatten()
            .join("\n\n")
    }

    fn header(&self) -> String {
        format!("TYPE {} (scalar)", self.detail.name)
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }
}

fn fields_summary(fields: &ExtendedFieldsDetail) -> String {
    format!(
        "{} fields\n{} deprecated fields",
        fields.field_count(),
        fields.deprecated_count
    )
}

fn fields_table(fields: &[FieldInfo]) -> Table {
    let mut table = Table::new();
    table.load_preset(presets::ASCII_FULL);
    table.set_header(["Field", "Type", "Description"]);

    for field in fields {
        let desc = match (
            &field.description,
            field.is_deprecated,
            &field.deprecation_reason,
        ) {
            (_, true, Some(reason)) => format!("(deprecated: {})", reason),
            (_, true, None) => "(deprecated)".to_string(),
            (Some(d), false, _) => d.clone(),
            (None, false, _) => String::new(),
        };
        table.add_row([field.name.as_str(), field.return_type.as_str(), &desc]);
    }

    table
}

fn input_fields_table(fields: &[InputFieldInfo]) -> Table {
    let mut table = Table::new();
    table.load_preset(presets::ASCII_FULL);
    table.set_header(["Field", "Type", "Description"]);

    for field in fields {
        let desc = match (
            &field.description,
            field.is_deprecated,
            &field.deprecation_reason,
        ) {
            (_, true, Some(reason)) => format!("(deprecated: {})", reason),
            (_, true, None) => "(deprecated)".to_string(),
            (Some(d), false, _) => d.clone(),
            (None, false, _) => String::new(),
        };
        table.add_row([field.name.as_str(), field.field_type.as_str(), &desc]);
    }

    table
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
    use apollo_compiler::Name;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::TypeDetailDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema, type_name: &str, include_deprecated: bool) -> String {
        let detail = schema
            .type_detail(&Name::new(type_name).unwrap(), include_deprecated, 0)
            .unwrap();
        TypeDetailDisplay::from(&detail).display()
    }

    // --- Object ---

    #[rstest]
    fn object_starts_with_header(schema: ParsedSchema) {
        assert_that!(display(&schema, "Post", true)).starts_with("TYPE Post (object)");
    }

    #[rstest]
    fn object_includes_implements_line(schema: ParsedSchema) {
        let out = display(&schema, "Post", true);
        assert_that!(out).contains("implements");
        assert_that!(out).contains("Node");
        assert_that!(out).contains("Timestamped");
    }

    #[rstest]
    fn object_no_implements_when_none(schema: ParsedSchema) {
        // Tag has no implements clause
        let out = display(&schema, "Tag", true);
        assert_that!(out).does_not_contain("implements");
    }

    #[rstest]
    fn object_fields_table_contains_field_names(schema: ParsedSchema) {
        let out = display(&schema, "Post", true);
        assert_that!(out).contains("Fields");
        assert_that!(out).contains("title");
        assert_that!(out).contains("body");
        assert_that!(out).contains("author");
    }

    #[rstest]
    fn object_deprecated_field_shown_with_reason(schema: ParsedSchema) {
        let out = display(&schema, "User", true);
        assert_that!(out).contains("deprecated: Use id instead");
    }

    #[rstest]
    fn object_deprecated_field_filtered_when_excluded(schema: ParsedSchema) {
        let out = display(&schema, "User", false);
        assert_that!(out).does_not_contain("legacyId");
    }

    #[rstest]
    fn object_includes_via_section(schema: ParsedSchema) {
        let out = display(&schema, "Post", true);
        assert_that!(out).contains("Available via:");
        assert_that!(out).contains("Query.post");
    }

    // --- Interface ---

    #[rstest]
    fn interface_starts_with_header(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Timestamped").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).starts_with("TYPE Timestamped (interface)");
    }

    #[rstest]
    fn interface_includes_implementors(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Timestamped").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("Implemented by:");
        assert_that!(out).contains("Post");
        assert_that!(out).contains("Comment");
    }

    #[rstest]
    fn interface_includes_fields(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("Timestamped").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("createdAt");
        assert_that!(out).contains("updatedAt");
    }

    // --- Input ---

    #[rstest]
    fn input_starts_with_header(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("CreatePostInput").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).starts_with("TYPE CreatePostInput (input)");
    }

    #[rstest]
    fn input_includes_fields(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("CreatePostInput").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("title");
        assert_that!(out).contains("body");
        assert_that!(out).contains("categoryId");
    }

    // --- Enum ---

    #[rstest]
    fn enum_starts_with_header(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("DigestFrequency").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).starts_with("TYPE DigestFrequency (enum)");
    }

    #[rstest]
    fn enum_values_table_contains_values(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("DigestFrequency").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("Values");
        assert_that!(out).contains("DAILY");
        assert_that!(out).contains("WEEKLY");
        assert_that!(out).contains("NEVER");
    }

    #[rstest]
    fn enum_summary_shows_deprecated_count(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("SortOrder").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("deprecated");
    }

    #[rstest]
    fn enum_deprecated_value_shows_reason(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("SortOrder").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("deprecated: Use TOP instead");
    }

    #[rstest]
    fn enum_deprecated_value_filtered_when_excluded(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("SortOrder").unwrap(), false, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).does_not_contain("RELEVANCE");
    }

    // --- Union ---

    #[rstest]
    fn union_starts_with_header(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("ContentItem").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).starts_with("TYPE ContentItem (union)");
    }

    #[rstest]
    fn union_includes_members(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("ContentItem").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).contains("Members:");
        assert_that!(out).contains("Post");
        assert_that!(out).contains("Comment");
    }

    // --- Scalar ---

    #[rstest]
    fn scalar_starts_with_header(schema: ParsedSchema) {
        let detail = schema
            .type_detail(&Name::new("DateTime").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).starts_with("TYPE DateTime (scalar)");
    }

    #[rstest]
    fn scalar_with_description(schema: ParsedSchema) {
        // URL has no description in the fixture — verify output is just the header
        let detail = schema
            .type_detail(&Name::new("URL").unwrap(), true, 0)
            .unwrap();
        let out = TypeDetailDisplay::from(&detail).display();
        assert_that!(out).is_equal_to("TYPE URL (scalar)".to_string());
    }
}
