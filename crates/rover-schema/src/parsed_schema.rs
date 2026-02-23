use apollo_compiler::Schema;

/// Wrapper around apollo_compiler::Schema providing convenient accessors.
pub struct ParsedSchema {
    schema: Schema,
}

impl ParsedSchema {
    /// Parse SDL into a schema. Uses permissive parsing (no validation)
    /// since we want to explore schemas that may have minor issues.
    pub fn parse(sdl: &str) -> Self {
        let schema = match Schema::parse(sdl, "schema.graphql") {
            Ok(schema) => schema,
            Err(with_errors) => with_errors.partial,
        };
        Self { schema }
    }

    pub const fn inner(&self) -> &Schema {
        &self.schema
    }
}
