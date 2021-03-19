use std::fmt::{self, Display};

/// Convenience Field Value implementation used when creating a Field.
/// Can be a `Type`, a `NonNull` or a `List`.
///
/// This enum is resposible for encoding creating values such as `String!`, `[[[[String]!]!]!]!`, etc.
///
/// ### Example
/// ```rust
/// use sdl_encoder::{FieldValue};
///
/// let field_ty = FieldValue::Type {
///     ty: "String".to_string(),
/// };
///
/// let list = FieldValue::List {
///     ty: Box::new(field_ty),
/// };
///
/// let non_null = FieldValue::NonNull { ty: Box::new(list) };
///
/// assert_eq!(non_null.to_string(), "[String]!");
/// ```
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
            FieldValue::Type { ty } => write!(f, "{}", ty),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encodes_simple_field_value() {
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
        };

        assert_eq!(field_ty.to_string(), "String");
    }

    #[test]
    fn encodes_list_field_value() {
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
        };

        let list = FieldValue::List {
            ty: Box::new(field_ty),
        };

        assert_eq!(list.to_string(), "[String]");
    }

    #[test]
    fn encodes_non_null_list_field_value() {
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
        };

        let list = FieldValue::List {
            ty: Box::new(field_ty),
        };

        let non_null = FieldValue::NonNull { ty: Box::new(list) };

        assert_eq!(non_null.to_string(), "[String]!");
    }
    #[test]
    fn encodes_non_null_list_non_null_list_field_value() {
        let field_ty = FieldValue::Type {
            ty: "String".to_string(),
        };

        let list = FieldValue::List {
            ty: Box::new(field_ty),
        };

        let non_null = FieldValue::NonNull { ty: Box::new(list) };

        let list_2 = FieldValue::List {
            ty: Box::new(non_null),
        };

        let non_null_2 = FieldValue::NonNull {
            ty: Box::new(list_2),
        };

        assert_eq!(non_null_2.to_string(), "[[String]!]!");
    }
}
