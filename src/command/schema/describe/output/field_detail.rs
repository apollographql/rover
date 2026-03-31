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

    #[rstest]
    fn full_output_query_post(schema: ParsedSchema) {
        assert_that!(display(&schema, "Query.post")).is_equal_to(
            "FIELD Query.post: Post\n\n\
             Get a post by ID\n\n\
             1 args\n\
             Args\n\
             +-----+------+-------+\n\
             | Arg | Type | Notes |\n\
             +====================+\n\
             | id  | ID   |       |\n\
             +-----+------+-------+\n\n\
             Return type: Post (object)\n\
             +--------------+-------------------+\n\
             | Field        | Type              |\n\
             +==================================+\n\
             | [implements] | Node              |\n\
             |              | Timestamped       |\n\
             |--------------+-------------------|\n\
             | id           | ID                |\n\
             |--------------+-------------------|\n\
             | title        | String            |\n\
             |--------------+-------------------|\n\
             | body         | String            |\n\
             |--------------+-------------------|\n\
             | author       | User              |\n\
             |--------------+-------------------|\n\
             | comments     | CommentConnection |\n\
             |--------------+-------------------|\n\
             | category     | Category          |\n\
             |--------------+-------------------|\n\
             | tags         | Tag               |\n\
             |--------------+-------------------|\n\
             | publishedAt  | String            |\n\
             |--------------+-------------------|\n\
             | createdAt    | String            |\n\
             |--------------+-------------------|\n\
             | updatedAt    | String            |\n\
             |--------------+-------------------|\n\
             | viewCount    | Int               |\n\
             |--------------+-------------------|\n\
             | score        | Int               |\n\
             |--------------+-------------------|\n\
             | slug         | String            |\n\
             |--------------+-------------------|\n\
             | oldSlug      | String            |\n\
             +--------------+-------------------+"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_user_posts(schema: ParsedSchema) {
        assert_that!(display(&schema, "User.posts")).is_equal_to(
            "FIELD User.posts: PostConnection\n\n\
             Posts authored by this user\n\n\
             2 args\n\
             Args\n\
             +--------+------+-------------------------------------------------+\n\
             | Arg    | Type | Notes                                           |\n\
             +=================================================================+\n\
             | limit  | Int  | Maximum number of posts to return (default: 20) |\n\
             |--------+------+-------------------------------------------------|\n\
             | offset | Int  |                                                 |\n\
             +--------+------+-------------------------------------------------+\n\n\
             Available via: Query.user, Mutation.createPost -> CreatePostPayload.post -> Post.author\n\n\
             Return type: PostConnection (object)\n\
             +----------+----------+\n\
             | Field    | Type     |\n\
             +=====================+\n\
             | edges    | PostEdge |\n\
             |----------+----------|\n\
             | pageInfo | PageInfo |\n\
             +----------+----------+"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_post_title(schema: ParsedSchema) {
        assert_that!(display(&schema, "Post.title")).is_equal_to(
            "FIELD Post.title: String!\n\n\
             Available via: Query.post, Mutation.createPost -> CreatePostPayload.post"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_post_body(schema: ParsedSchema) {
        assert_that!(display(&schema, "Post.body")).is_equal_to(
            "FIELD Post.body: String!\n\n\
             The body content\n\n\
             Available via: Query.post, Mutation.createPost -> CreatePostPayload.post"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_post_old_slug(schema: ParsedSchema) {
        assert_that!(display(&schema, "Post.oldSlug")).is_equal_to(
            "FIELD Post.oldSlug: String\n\n\
             DEPRECATED: Use slug instead\n\n\
             Available via: Query.post, Mutation.createPost -> CreatePostPayload.post"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_mutation_create_post(schema: ParsedSchema) {
        assert_that!(display(&schema, "Mutation.createPost")).is_equal_to(
            "FIELD Mutation.createPost: CreatePostPayload\n\n\
             Create a new post\n\n\
             1 args\n\
             Args\n\
             +-------+-----------------+-------+\n\
             | Arg   | Type            | Notes |\n\
             +=================================+\n\
             | input | CreatePostInput |       |\n\
             +-------+-----------------+-------+\n\n\
             Return type: CreatePostPayload (object)\n\
             +-------+------+\n\
             | Field | Type |\n\
             +==============+\n\
             | post  | Post |\n\
             +-------+------+\n\n\
             Input types\n\
             CreatePostInput (input)\n\
             +------------+--------+\n\
             | Field      | Type   |\n\
             +=====================+\n\
             | title      | String |\n\
             |------------+--------|\n\
             | body       | String |\n\
             |------------+--------|\n\
             | categoryId | ID     |\n\
             |------------+--------|\n\
             | tags       | String |\n\
             +------------+--------+"
                .to_string(),
        );
    }
}
