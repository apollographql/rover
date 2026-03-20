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
        let parts = vec![
            format!("{} types", ov.total_types),
            format!("{} fields", ov.total_fields),
            format!("{} deprecated fields", ov.total_deprecated),
        ];
        parts.join("\n")
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
