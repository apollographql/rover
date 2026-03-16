pub mod describe;
pub mod error;
pub mod format;
pub mod parsed_schema;
pub mod root_paths;
pub mod schema_source;
pub(crate) mod util;

// Re-export main public types
pub use apollo_compiler::coordinate::SchemaCoordinate;
pub use describe::{
    DescribeResult, EnumDetail, ExtendedFieldsDetail, FieldDetail, FieldsDetail, InputDetail,
    InterfaceDetail, ObjectDetail, ScalarDetail, SchemaOverview, TypeDetail, UnionDetail,
};
pub use error::SchemaError;
pub use format::OutputFormat;
pub use parsed_schema::ParsedSchema;
