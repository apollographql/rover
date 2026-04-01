use comfy_table::{Table, presets};
use itertools::Itertools;
use rover_schema::SchemaOverview;

pub struct SchemaOverviewDisplay<'a> {
    overview: &'a SchemaOverview,
}

impl<'a> SchemaOverviewDisplay<'a> {
    pub fn display(&self) -> String {
        [
            Some(self.header()),
            Some(self.subheader()),
            self.operations(),
            Some(self.types()),
        ]
        .into_iter()
        .flatten()
        .join("\n\n")
    }

    fn header(&self) -> String {
        format!("SCHEMA {}", self.overview.schema_source)
    }

    fn subheader(&self) -> String {
        let ov = self.overview;
        [
            format!("{} types", ov.total_types),
            format!("{} fields", ov.total_fields),
            format!("{} deprecated fields", ov.total_deprecated),
        ]
        .join("\n")
    }

    fn operations(&self) -> Option<String> {
        let ov = self.overview;
        if ov.query_fields.is_empty() && ov.mutation_fields.is_empty() {
            return None;
        }

        let mut table = Table::new();
        table.load_preset(presets::ASCII_FULL);
        table.set_header(["Type", "#", "Fields"]);

        if !ov.query_fields.is_empty() {
            let names = ov.query_fields.iter().map(|f| f.name.as_str()).join("\n");
            table.add_row(["Query", &ov.query_fields.len().to_string(), &names]);
        }
        if !ov.mutation_fields.is_empty() {
            let names = ov
                .mutation_fields
                .iter()
                .map(|f| f.name.as_str())
                .join("\n");
            table.add_row(["Mutation", &ov.mutation_fields.len().to_string(), &names]);
        }

        Some(format!("Operations\n{table}"))
    }

    fn types(&self) -> String {
        let ov = self.overview;
        let mut table = Table::new();
        table.load_preset(presets::ASCII_FULL);
        table.set_header(["Kind", "#", "Names"]);

        for (kind, names) in [
            ("objects", &ov.objects),
            ("inputs", &ov.inputs),
            ("enums", &ov.enums),
            ("interfaces", &ov.interfaces),
            ("unions", &ov.unions),
            ("scalars", &ov.scalars),
        ] {
            if !names.is_empty() {
                let names_str = names.iter().map(|n| n.as_str()).join("\n");
                table.add_row([kind, &names.len().to_string(), &names_str]);
            }
        }

        format!("Types\n{table}")
    }
}

impl<'a> From<&'a SchemaOverview> for SchemaOverviewDisplay<'a> {
    fn from(overview: &'a SchemaOverview) -> Self {
        SchemaOverviewDisplay { overview }
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rover_schema::ParsedSchema;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::SchemaOverviewDisplay;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!(
            "../../../../../crates/rover-schema/src/test_fixtures/test_schema.graphql"
        );
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn display(schema: &ParsedSchema) -> String {
        let overview = schema.overview();
        SchemaOverviewDisplay::from(&overview).display()
    }

    #[rstest]
    fn full_output(schema: ParsedSchema) {
        assert_that!(display(&schema)).is_equal_to(
            indoc! {"
                SCHEMA test_schema.graphql

                30 types
                86 fields
                3 deprecated fields

                Operations
                +----------+---+-------------------+
                | Type     | # | Fields            |
                +==================================+
                | Query    | 5 | user              |
                |          |   | post              |
                |          |   | categories        |
                |          |   | search            |
                |          |   | viewer            |
                |----------+---+-------------------|
                | Mutation | 3 | createPost        |
                |          |   | updatePreferences |
                |          |   | deleteComment     |
                +----------+---+-------------------+

                Types
                +------------+----+--------------------------+
                | Kind       | #  | Names                    |
                +============================================+
                | objects    | 16 | Category                 |
                |            |    | Comment                  |
                |            |    | CommentConnection        |
                |            |    | CommentEdge              |
                |            |    | CreatePostPayload        |
                |            |    | DeleteCommentPayload     |
                |            |    | PageInfo                 |
                |            |    | Post                     |
                |            |    | PostConnection           |
                |            |    | PostEdge                 |
                |            |    | Preferences              |
                |            |    | SearchResults            |
                |            |    | Tag                      |
                |            |    | UpdatePreferencesPayload |
                |            |    | User                     |
                |            |    | Viewer                   |
                |------------+----+--------------------------|
                | inputs     | 2  | CreatePostInput          |
                |            |    | UpdatePreferencesInput   |
                |------------+----+--------------------------|
                | enums      | 4  | DigestFrequency          |
                |            |    | Role                     |
                |            |    | SearchType               |
                |            |    | SortOrder                |
                |------------+----+--------------------------|
                | interfaces | 3  | Node                     |
                |            |    | Profile                  |
                |            |    | Timestamped              |
                |------------+----+--------------------------|
                | unions     | 1  | ContentItem              |
                |------------+----+--------------------------|
                | scalars    | 2  | DateTime                 |
                |            |    | URL                      |
                +------------+----+--------------------------+"}
            .to_string(),
        );
    }
}
