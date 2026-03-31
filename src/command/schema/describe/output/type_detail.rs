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
    fn full_output_object_post(schema: ParsedSchema) {
        assert_that!(display(&schema, "Post", true)).is_equal_to(
            "TYPE Post (object)\n\n\
             A content post\n\n\
             implements Node, Timestamped\n\n\
             14 fields\n\
             1 deprecated fields\n\n\
             Fields\n\
             +-------------+-------------------+-------------------------------------+\n\
             | Field       | Type              | Description                         |\n\
             +=======================================================================+\n\
             | id          | ID                |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | title       | String            |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | body        | String            | The body content                    |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | author      | User              | The author of this post             |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | comments    | CommentConnection |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | category    | Category          |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | tags        | Tag               |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | publishedAt | String            |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | createdAt   | String            |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | updatedAt   | String            |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | viewCount   | Int               |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | score       | Int               | The post's popularity score (0-100) |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | slug        | String            |                                     |\n\
             |-------------+-------------------+-------------------------------------|\n\
             | oldSlug     | String            | (deprecated: Use slug instead)      |\n\
             +-------------+-------------------+-------------------------------------+\n\n\
             Available via: Query.post, Mutation.createPost -> CreatePostPayload.post"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_object_user_with_deprecated(schema: ParsedSchema) {
        assert_that!(display(&schema, "User", true)).is_equal_to(
            "TYPE User (object)\n\n\
             A registered user\n\n\
             implements Node, Profile\n\n\
             8 fields\n\
             1 deprecated fields\n\n\
             Fields\n\
             +-----------+----------------+------------------------------+\n\
             | Field     | Type           | Description                  |\n\
             +===========================================================+\n\
             | id        | ID             |                              |\n\
             |-----------+----------------+------------------------------|\n\
             | name      | String         |                              |\n\
             |-----------+----------------+------------------------------|\n\
             | email     | String         | The user's email address     |\n\
             |-----------+----------------+------------------------------|\n\
             | posts     | PostConnection | Posts authored by this user  |\n\
             |-----------+----------------+------------------------------|\n\
             | bio       | String         |                              |\n\
             |-----------+----------------+------------------------------|\n\
             | avatarUrl | String         |                              |\n\
             |-----------+----------------+------------------------------|\n\
             | createdAt | String         |                              |\n\
             |-----------+----------------+------------------------------|\n\
             | legacyId  | String         | (deprecated: Use id instead) |\n\
             +-----------+----------------+------------------------------+\n\n\
             Available via: Query.user, Mutation.createPost -> CreatePostPayload.post -> Post.author"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_object_user_deprecated_excluded(schema: ParsedSchema) {
        assert_that!(display(&schema, "User", false)).is_equal_to(
            "TYPE User (object)\n\n\
             A registered user\n\n\
             implements Node, Profile\n\n\
             8 fields\n\
             1 deprecated fields\n\n\
             Fields\n\
             +-----------+----------------+-----------------------------+\n\
             | Field     | Type           | Description                 |\n\
             +==========================================================+\n\
             | id        | ID             |                             |\n\
             |-----------+----------------+-----------------------------|\n\
             | name      | String         |                             |\n\
             |-----------+----------------+-----------------------------|\n\
             | email     | String         | The user's email address    |\n\
             |-----------+----------------+-----------------------------|\n\
             | posts     | PostConnection | Posts authored by this user |\n\
             |-----------+----------------+-----------------------------|\n\
             | bio       | String         |                             |\n\
             |-----------+----------------+-----------------------------|\n\
             | avatarUrl | String         |                             |\n\
             |-----------+----------------+-----------------------------|\n\
             | createdAt | String         |                             |\n\
             +-----------+----------------+-----------------------------+\n\n\
             Available via: Query.user, Mutation.createPost -> CreatePostPayload.post -> Post.author"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_object_tag(schema: ParsedSchema) {
        assert_that!(display(&schema, "Tag", true)).is_equal_to(
            "TYPE Tag (object)\n\n\
             A tag applied to posts\n\n\
             2 fields\n\
             0 deprecated fields\n\n\
             Fields\n\
             +-----------+--------+-------------+\n\
             | Field     | Type   | Description |\n\
             +==================================+\n\
             | name      | String |             |\n\
             |-----------+--------+-------------|\n\
             | postCount | Int    |             |\n\
             +-----------+--------+-------------+\n\n\
             Available via: Query.post -> Post.tags, Mutation.createPost -> CreatePostPayload.post -> Post.tags"
                .to_string(),
        );
    }

    // --- Interface ---

    #[rstest]
    fn full_output_interface_timestamped(schema: ParsedSchema) {
        assert_that!(display(&schema, "Timestamped", true)).is_equal_to(
            "TYPE Timestamped (interface)\n\n\
             An entity with timestamps\n\n\
             2 fields\n\
             0 deprecated fields\n\n\
             Fields\n\
             +-----------+--------+-------------+\n\
             | Field     | Type   | Description |\n\
             +==================================+\n\
             | createdAt | String |             |\n\
             |-----------+--------+-------------|\n\
             | updatedAt | String |             |\n\
             +-----------+--------+-------------+\n\n\
             Implemented by: Post, Comment"
                .to_string(),
        );
    }

    // --- Input ---

    #[rstest]
    fn full_output_input_create_post(schema: ParsedSchema) {
        assert_that!(display(&schema, "CreatePostInput", true)).is_equal_to(
            "TYPE CreatePostInput (input)\n\n\
             4 fields\n\n\
             Fields\n\
             +------------+--------+----------------+\n\
             | Field      | Type   | Description    |\n\
             +======================================+\n\
             | title      | String | The post title |\n\
             |------------+--------+----------------|\n\
             | body       | String | The post body  |\n\
             |------------+--------+----------------|\n\
             | categoryId | ID     | Category ID    |\n\
             |------------+--------+----------------|\n\
             | tags       | String | Optional tags  |\n\
             +------------+--------+----------------+"
                .to_string(),
        );
    }

    // --- Enum ---

    #[rstest]
    fn full_output_enum_digest_frequency(schema: ParsedSchema) {
        assert_that!(display(&schema, "DigestFrequency", true)).is_equal_to(
            "TYPE DigestFrequency (enum)\n\n\
             Email digest frequency\n\n\
             3 values\n\n\
             Values\n\
             +--------+-------------+\n\
             | Value  | Description |\n\
             +======================+\n\
             | DAILY  |             |\n\
             |--------+-------------|\n\
             | WEEKLY |             |\n\
             |--------+-------------|\n\
             | NEVER  |             |\n\
             +--------+-------------+\n\n\
             Available via: Query.viewer -> Viewer.preferences -> Preferences.digestFrequency, \
             Mutation.updatePreferences -> UpdatePreferencesPayload.preferences -> Preferences.digestFrequency"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_enum_sort_order_with_deprecated(schema: ParsedSchema) {
        assert_that!(display(&schema, "SortOrder", true)).is_equal_to(
            "TYPE SortOrder (enum)\n\n\
             4 values\n\
             1 deprecated values\n\n\
             Values\n\
             +-----------+-------------------------------+\n\
             | Value     | Description                   |\n\
             +===========================================+\n\
             | NEWEST    |                               |\n\
             |-----------+-------------------------------|\n\
             | OLDEST    |                               |\n\
             |-----------+-------------------------------|\n\
             | TOP       |                               |\n\
             |-----------+-------------------------------|\n\
             | RELEVANCE | (deprecated: Use TOP instead) |\n\
             +-----------+-------------------------------+"
                .to_string(),
        );
    }

    #[rstest]
    fn full_output_enum_sort_order_deprecated_excluded(schema: ParsedSchema) {
        assert_that!(display(&schema, "SortOrder", false)).is_equal_to(
            "TYPE SortOrder (enum)\n\n\
             4 values\n\
             1 deprecated values\n\n\
             Values\n\
             +--------+-------------+\n\
             | Value  | Description |\n\
             +======================+\n\
             | NEWEST |             |\n\
             |--------+-------------|\n\
             | OLDEST |             |\n\
             |--------+-------------|\n\
             | TOP    |             |\n\
             +--------+-------------+"
                .to_string(),
        );
    }

    // --- Union ---

    #[rstest]
    fn full_output_union_content_item(schema: ParsedSchema) {
        assert_that!(display(&schema, "ContentItem", true))
            .is_equal_to("TYPE ContentItem (union)\n\nMembers: Post, Comment".to_string());
    }

    // --- Scalar ---

    #[rstest]
    fn full_output_scalar_date_time(schema: ParsedSchema) {
        assert_that!(display(&schema, "DateTime", true))
            .is_equal_to("TYPE DateTime (scalar)".to_string());
    }

    #[rstest]
    fn full_output_scalar_url(schema: ParsedSchema) {
        assert_that!(display(&schema, "URL", true))
            .is_equal_to("TYPE URL (scalar)".to_string());
    }
}
