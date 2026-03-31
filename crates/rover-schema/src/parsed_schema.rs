use std::path::{Path, PathBuf};

pub use apollo_compiler::schema::ExtendedType;
use apollo_compiler::{Schema, coordinate::SchemaCoordinate, parser::FileId};

/// Wrapper around apollo_compiler::Schema providing convenient accessors.
pub struct ParsedSchema {
    schema: Schema,
}

impl ParsedSchema {
    /// Parse SDL into a schema. Uses permissive parsing (no validation)
    /// since we want to explore schemas that may have minor issues.
    pub fn parse(sdl: &str, path: impl AsRef<Path>) -> Self {
        let schema = match Schema::parse(sdl, path) {
            Ok(schema) => schema,
            Err(with_errors) => with_errors.partial,
        };
        Self { schema }
    }

    /// Returns the path this schema was parsed from, skipping the apollo built-in source.
    pub fn source_path(&self) -> Option<PathBuf> {
        self.schema
            .sources
            .iter()
            .find(|(id, _)| **id != FileId::BUILT_IN)
            .map(|(_, s)| s.path().to_path_buf())
    }

    pub(crate) const fn inner(&self) -> &Schema {
        &self.schema
    }

    /// Returns SDL for the schema filtered to the type referenced by `coord`, or `None` if the
    /// coordinate is unsupported or the type is not found. Returns the full schema SDL when
    /// `coord` is `None`.
    pub fn filtered_sdl(&self, coord: Option<&SchemaCoordinate>) -> Option<String> {
        let schema = self.inner();
        let Some(coord) = coord else {
            return Some(schema.serialize().to_string());
        };

        let type_name = match coord {
            SchemaCoordinate::Type(tc) => &tc.ty,
            SchemaCoordinate::TypeAttribute(tac) => &tac.ty,
            SchemaCoordinate::FieldArgument(fac) => &fac.ty,
            SchemaCoordinate::Directive(_) | SchemaCoordinate::DirectiveArgument(_) => {
                return None;
            }
        };

        schema
            .types
            .get(type_name)
            .map(|ty| ty.serialize().to_string())
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::coordinate::SchemaCoordinate;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::ParsedSchema;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn filtered(schema: &ParsedSchema, coord: Option<&str>) -> Option<String> {
        let parsed_coord = coord.map(|s| s.parse::<SchemaCoordinate>().unwrap());
        schema.filtered_sdl(parsed_coord.as_ref())
    }

    const USER_SDL: &str = r#""""A registered user"""
type User implements Node & Profile {
  id: ID!
  name: String!
  """The user's email address"""
  email: String!
  """Posts authored by this user"""
  posts(
    """Maximum number of posts to return"""
    limit: Int = 20,
    offset: Int,
  ): PostConnection
  bio: String
  avatarUrl: String
  createdAt: String!
  legacyId: String @deprecated(reason: "Use id instead")
}
"#;

    // --- No coordinate — full schema SDL ---

    #[rstest]
    fn no_coord_includes_all_top_level_definitions(schema: ParsedSchema) {
        let out = filtered(&schema, None).unwrap();
        assert_that!(out).contains("type Query");
        assert_that!(out).contains("type User");
        assert_that!(out).contains("type Post");
        assert_that!(out).contains("directive @auth");
        assert_that!(out).contains("enum Role");
        assert_that!(out).contains("scalar DateTime");
        assert_that!(out).contains("union ContentItem");
    }

    // --- Coordinates that return the User type SDL ---

    #[rstest]
    #[case("User")]
    #[case("User.posts")]
    #[case("User.posts(limit:)")]
    fn coord_returns_user_sdl(schema: ParsedSchema, #[case] coord: &str) {
        assert_that!(filtered(&schema, Some(coord))).is_equal_to(Some(USER_SDL.to_string()));
    }

    // --- Coordinates that return None ---

    #[rstest]
    #[case("@auth")]
    #[case("@auth(requires:)")]
    #[case("NonExistent")]
    fn coord_returns_none(schema: ParsedSchema, #[case] coord: &str) {
        assert_that!(filtered(&schema, Some(coord))).is_none();
    }
}
