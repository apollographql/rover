use apollo_compiler::Name;

/// Metadata about a single value within a GraphQL enum.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EnumValueInfo {
    /// The enum value name.
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Whether this value is marked `@deprecated`.
    pub is_deprecated: bool,
    /// The reason given for deprecation, if any.
    pub deprecation_reason: Option<String>,
}
