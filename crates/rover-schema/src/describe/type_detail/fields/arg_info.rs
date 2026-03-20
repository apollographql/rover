use apollo_compiler::Name;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ArgInfo {
    pub name: Name,
    pub arg_type: Name,
    pub description: Option<String>,
    pub default_value: Option<String>,
}
