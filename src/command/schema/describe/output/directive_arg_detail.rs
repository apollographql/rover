use itertools::Itertools;
use rover_schema::DirectiveArgDetail;

pub struct DirectiveArgDetailDisplay<'a> {
    detail: &'a DirectiveArgDetail,
}

impl<'a> DirectiveArgDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [Some(self.header()), self.description(), self.default_value()]
            .into_iter()
            .flatten()
            .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        format!(
            "DIRECTIVE ARG @{}({}:): {}",
            d.directive_name, d.arg_name, d.arg_type
        )
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn default_value(&self) -> Option<String> {
        self.detail
            .default_value
            .as_deref()
            .map(|v| format!("Default: {}", v))
    }
}

impl<'a> From<&'a DirectiveArgDetail> for DirectiveArgDetailDisplay<'a> {
    fn from(detail: &'a DirectiveArgDetail) -> Self {
        DirectiveArgDetailDisplay { detail }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::DirectiveArgDetailDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema, coord: &str) -> String {
        let coord: SchemaCoordinate = coord.parse().unwrap();
        let SchemaCoordinate::DirectiveArgument(ref dac) = coord else {
            panic!("expected a directive argument coordinate");
        };
        let detail = schema
            .directive_arg_detail(&dac.directive, &dac.argument)
            .unwrap();
        DirectiveArgDetailDisplay::from(&detail).display()
    }

    #[rstest]
    fn full_output(schema: ParsedSchema) {
        let out = display(&schema, "@auth(requires:)");
        assert_that!(out).is_equal_to(
            "DIRECTIVE ARG @auth(requires:): Role\n\n\
             The minimum role required to access this field\n\n\
             Default: USER"
                .to_string(),
        );
    }
}
