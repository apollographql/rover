//! Types and utilities for parsing and querying GraphQL schemas.

#![warn(missing_docs)]

/// Schema description and introspection utilities.
pub mod describe;
/// Error types for schema operations.
pub mod error;
/// Parsed schema wrapper.
pub mod parsed_schema;
/// Root-path traversal for finding how types are reachable.
pub mod root_paths;
// Re-export main public types
pub use apollo_compiler::coordinate::SchemaCoordinate;
pub use describe::{
    DescribeOutput, DirectiveArgDetail, DirectiveDetail, EnumDetail, ExtendedFieldsDetail,
    FieldArgDetail, FieldDetail, FieldsDetail, InputDetail, InputFieldInfo, InterfaceDetail,
    ObjectDetail, ScalarDetail, SchemaOverview, TypeDetail, UnionDetail,
};
pub use error::SchemaError;
pub use parsed_schema::ParsedSchema;
