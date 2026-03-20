use apollo_compiler::Name;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EnumValueInfo {
    pub name: Name,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}
