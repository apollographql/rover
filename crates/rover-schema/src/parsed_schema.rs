use std::path::{Path, PathBuf};

pub use apollo_compiler::schema::ExtendedType;
use apollo_compiler::{Schema, coordinate::SchemaCoordinate};

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

    /// Returns the path this schema was parsed from.
    pub fn source_path(&self) -> Option<PathBuf> {
        self.schema
            .sources
            .values()
            .next()
            .map(|s| s.path().to_path_buf())
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
