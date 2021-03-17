use std::fmt::{self, Display};

/// Define Field Type.
#[derive(Debug, PartialEq, Clone)]
pub enum FieldType {
    /// The non-null field type.
    NonNull {
        /// Null inner type.
        ty: Box<FieldType>,
    },
    /// The list field type.
    List {
        /// List inner type.
        ty: Box<FieldType>,
    },
    /// The type field type.
    Type {
        /// Type type.
        ty: String,
        /// Default field type type.
        default: Option<Box<FieldType>>,
    },
}

impl Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldType::List { ty } => {
                write!(f, "[{}]", ty)
            }
            FieldType::NonNull { ty } => {
                write!(f, "{}!", ty)
            }
            FieldType::Type {
                ty,
                // TODO(@lrlna): figure out the best way to encode default
                // values in fields
                default: _,
            } => {
                write!(f, "{}", ty)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encodes_simple_field_type() {
        let field_ty = FieldType::Type {
            ty: "String".to_string(),
            default: None,
        };

        assert_eq!(field_ty.to_string(), "String");
    }

    #[test]
    fn encodes_list_field() {
        let field_ty = FieldType::Type {
            ty: "String".to_string(),
            default: None,
        };

        let list = FieldType::List {
            ty: Box::new(field_ty),
        };

        assert_eq!(list.to_string(), "[String]");
    }
}
