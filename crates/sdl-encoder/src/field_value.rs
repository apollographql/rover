use std::fmt::{self, Display};

/// Define Field Type.
#[derive(Debug, PartialEq, Clone)]
pub enum FieldValue {
    /// The non-null field type.
    NonNull {
        /// Null inner type.
        ty: Box<FieldValue>,
    },
    /// The list field type.
    List {
        /// List inner type.
        ty: Box<FieldValue>,
    },
    /// The type field type.
    Type {
        /// Type type.
        ty: String,
        /// Default field type type.
        default: Option<Box<FieldValue>>,
    },
}

impl Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldValue::List { ty } => {
                write!(f, "[{}]", ty)
            }
            FieldValue::NonNull { ty } => {
                write!(f, "{}!", ty)
            }
            FieldValue::Type {
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
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
            default: None,
        };

        assert_eq!(field_ty.to_string(), "String");
    }

    #[test]
    fn encodes_list_field() {
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
            default: None,
        };

        let list = FieldValue::List {
            ty: Box::new(field_ty),
        };

        assert_eq!(list.to_string(), "[String]");
    }
}
