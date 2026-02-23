use crate::error::SchemaError;

/// A parsed schema coordinate: either a type name or a type.field reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaCoordinate {
    /// Just a type name, e.g. "Post"
    Type(String),
    /// A type and field, e.g. "User.posts"
    Field {
        type_name: String,
        field_name: String,
    },
}

impl SchemaCoordinate {
    pub fn parse(s: &str) -> Result<Self, SchemaError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(SchemaError::InvalidCoordinate(
                "coordinate cannot be empty".into(),
            ));
        }

        if let Some(dot_pos) = s.find('.') {
            let type_name = &s[..dot_pos];
            let field_name = &s[dot_pos + 1..];
            if type_name.is_empty() || field_name.is_empty() {
                return Err(SchemaError::InvalidCoordinate(format!(
                    "invalid coordinate '{s}': both type and field must be non-empty"
                )));
            }
            Ok(SchemaCoordinate::Field {
                type_name: type_name.to_string(),
                field_name: field_name.to_string(),
            })
        } else {
            Ok(SchemaCoordinate::Type(s.to_string()))
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            SchemaCoordinate::Type(name) => name,
            SchemaCoordinate::Field { type_name, .. } => type_name,
        }
    }
}

impl std::fmt::Display for SchemaCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaCoordinate::Type(name) => write!(f, "{name}"),
            SchemaCoordinate::Field {
                type_name,
                field_name,
            } => write!(f, "{type_name}.{field_name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_type_coordinate() {
        let coord = SchemaCoordinate::parse("Post").unwrap();
        assert_eq!(coord, SchemaCoordinate::Type("Post".into()));
    }

    #[test]
    fn parse_field_coordinate() {
        let coord = SchemaCoordinate::parse("User.posts").unwrap();
        assert_eq!(
            coord,
            SchemaCoordinate::Field {
                type_name: "User".into(),
                field_name: "posts".into()
            }
        );
    }

    #[test]
    fn reject_empty() {
        assert!(SchemaCoordinate::parse("").is_err());
    }

    #[test]
    fn reject_dot_only() {
        assert!(SchemaCoordinate::parse(".").is_err());
    }

    #[test]
    fn reject_trailing_dot() {
        assert!(SchemaCoordinate::parse("Post.").is_err());
    }

    #[test]
    fn reject_leading_dot() {
        assert!(SchemaCoordinate::parse(".field").is_err());
    }

    #[test]
    fn display_type() {
        let coord = SchemaCoordinate::Type("Post".into());
        assert_eq!(coord.to_string(), "Post");
    }

    #[test]
    fn display_field() {
        let coord = SchemaCoordinate::Field {
            type_name: "User".into(),
            field_name: "posts".into(),
        };
        assert_eq!(coord.to_string(), "User.posts");
    }
}
