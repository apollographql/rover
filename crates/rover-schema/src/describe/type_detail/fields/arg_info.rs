use apollo_compiler::Name;

/// Metadata about a single argument on a field.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ArgInfo {
    /// The argument name.
    pub name: Name,
    /// The inner named type of the argument.
    pub arg_type: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// The default value as a string, if one is specified.
    pub default_value: Option<String>,
}
