use apollo_compiler::Schema;
use apollo_compiler::coordinate::SchemaCoordinate;
pub use apollo_compiler::schema::ExtendedType;

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

    /// Returns the `ExtendedType` referenced by `coord`, if any.
    ///
    /// Returns `None` when `coord` is `None`, a directive coordinate, or names a type
    /// not present in the schema. The caller can fall back to `self.schema.serialize()`
    /// for the full SDL in those cases.
    pub fn filter(&self, coord: Option<&SchemaCoordinate>) -> Option<&ExtendedType> {
        let type_name = match coord? {
            SchemaCoordinate::Type(tc) => &tc.ty,
            SchemaCoordinate::TypeAttribute(tac) => &tac.ty,
            SchemaCoordinate::FieldArgument(fac) => &fac.ty,
            SchemaCoordinate::Directive(_) | SchemaCoordinate::DirectiveArgument(_) => {
                return None;
            }
        };

        self.schema.types.get(type_name)
    }
}
