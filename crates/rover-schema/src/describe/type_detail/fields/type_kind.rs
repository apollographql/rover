#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind {
    Object,
    Input,
    Enum,
    Interface,
    Union,
    Scalar,
}

impl std::fmt::Display for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TypeKind::Object => "object",
            TypeKind::Input => "input",
            TypeKind::Enum => "enum",
            TypeKind::Interface => "interface",
            TypeKind::Union => "union",
            TypeKind::Scalar => "scalar",
        };
        write!(f, "{s}")
    }
}
