use comfy_table::{Table, presets};
use itertools::Itertools;
use rover_schema::DirectiveDetail;

pub struct DirectiveDetailDisplay<'a> {
    detail: &'a DirectiveDetail,
}

impl<'a> DirectiveDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [
            Some(self.header()),
            self.description(),
            self.locations(),
            self.args(),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        if d.repeatable {
            format!("DIRECTIVE @{} repeatable", d.name)
        } else {
            format!("DIRECTIVE @{}", d.name)
        }
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn locations(&self) -> Option<String> {
        if self.detail.locations.is_empty() {
            return None;
        }
        Some(format!(
            "Locations: {}",
            self.detail.locations.iter().join(", ")
        ))
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

        Some(format!("Args\n{table}"))
    }
}

impl<'a> From<&'a DirectiveDetail> for DirectiveDetailDisplay<'a> {
    fn from(detail: &'a DirectiveDetail) -> Self {
        DirectiveDetailDisplay { detail }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::DirectiveDetailDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema, coord: &str) -> String {
        let coord: SchemaCoordinate = coord.parse().unwrap();
        let SchemaCoordinate::Directive(ref dc) = coord else {
            panic!("expected a directive coordinate");
        };
        let detail = schema.directive_detail(&dc.directive).unwrap();
        DirectiveDetailDisplay::from(&detail).display()
    }

    #[rstest]
    fn full_output(schema: ParsedSchema) {
        assert_that!(display(&schema, "@auth")).is_equal_to(
            "DIRECTIVE @auth\n\n\
             Marks a field or object as requiring a minimum role\n\n\
             Locations: FIELD_DEFINITION, OBJECT\n\n\
             Args\n\
             +----------+------+----------------------------------------------------------------+\n\
             | Arg      | Type | Notes                                                          |\n\
             +==================================================================================+\n\
             | requires | Role | The minimum role required to access this field (default: USER) |\n\
             +----------+------+----------------------------------------------------------------+"
                .to_string(),
        );
    }
}
