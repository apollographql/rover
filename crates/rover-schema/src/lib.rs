pub mod coordinate;
pub mod describe;
pub mod error;
pub mod format;
pub mod parsed_schema;
pub mod root_paths;
#[cfg(feature = "search")]
pub mod search;

// Re-export main public types
pub use coordinate::SchemaCoordinate;
pub use describe::{DescribeResult, FieldDetail, SchemaOverview, TypeDetail};
pub use error::SchemaError;
pub use format::OutputFormat;
pub use parsed_schema::ParsedSchema;
