use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Failed to parse schema: {0}")]
    ParseError(String),

    #[error("Type not found: {0}")]
    TypeNotFound(String),

    #[error("Field '{field}' not found on type '{type_name}'")]
    FieldNotFound { type_name: String, field: String },

    #[error("Invalid coordinate: {0}")]
    InvalidCoordinate(String),

    #[cfg(feature = "search")]
    #[error("Search index error: {0}")]
    SearchError(String),
}
