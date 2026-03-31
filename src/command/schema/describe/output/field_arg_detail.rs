use itertools::Itertools;
use rover_schema::FieldArgDetail;

pub struct FieldArgDetailDisplay<'a> {
    detail: &'a FieldArgDetail,
}

impl<'a> FieldArgDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [Some(self.header()), self.description(), self.default_value()]
            .into_iter()
            .flatten()
            .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        format!(
            "FIELD ARG {}.{}({}:): {}",
            d.type_name, d.field_name, d.arg_name, d.arg_type
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

impl<'a> From<&'a FieldArgDetail> for FieldArgDetailDisplay<'a> {
    fn from(detail: &'a FieldArgDetail) -> Self {
        FieldArgDetailDisplay { detail }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::FieldArgDetailDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema, coord: &str) -> String {
        let coord: SchemaCoordinate = coord.parse().unwrap();
        let SchemaCoordinate::FieldArgument(ref fac) = coord else {
            panic!("expected a field argument coordinate");
        };
        let detail = schema.field_arg_detail(fac).unwrap();
        FieldArgDetailDisplay::from(&detail).display()
    }

    #[rstest]
    fn full_output_with_description_and_default(schema: ParsedSchema) {
        assert_that!(display(&schema, "User.posts(limit:)")).is_equal_to(
            "FIELD ARG User.posts(limit:): Int\n\n\
             Maximum number of posts to return\n\n\
             Default: 20"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_without_description_or_default(schema: ParsedSchema) {
        assert_that!(display(&schema, "User.posts(offset:)"))
            .is_equal_to("FIELD ARG User.posts(offset:): Int".to_string());
    }
}
