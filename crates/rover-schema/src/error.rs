use apollo_compiler::Name;
use apollo_compiler::coordinate::SchemaCoordinate;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Type not found: {0}")]
    TypeNotFound(Name),

    #[error("Field '{field}' not found on type '{type_name}'")]
    FieldNotFound { type_name: Name, field: Name },

    #[error("Invalid coordinate: {0}")]
    InvalidCoordinate(SchemaCoordinate),
}
