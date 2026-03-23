use apollo_compiler::{Name, coordinate::SchemaCoordinate};
use thiserror::Error;

/// Errors that can occur during schema lookup and describe operations.
#[derive(Error, Debug)]
pub enum SchemaError {
    /// The requested type does not exist in the schema.
    #[error("Type not found: {0}")]
    TypeNotFound(Name),

    /// The requested field does not exist on the given type.
    #[error("Field '{field}' not found on type '{type_name}'")]
    FieldNotFound {
        /// The name of the type that was searched.
        type_name: Name,
        /// The field name that was not found.
        field: Name,
    },

    /// The schema coordinate kind is not supported by this operation.
    #[error("Unsupported coordinate: {0}")]
    UnsupportedCoordinate(SchemaCoordinate),
}
