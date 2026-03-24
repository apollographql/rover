use std::path::{Path, PathBuf};

use apollo_compiler::Schema;
pub use apollo_compiler::schema::ExtendedType;

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
}
