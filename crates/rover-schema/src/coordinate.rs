use crate::error::SchemaError;

/// A parsed schema coordinate per the
/// [Schema Coordinates RFC](https://github.com/graphql/graphql-wg/blob/main/rfcs/SchemaCoordinates.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaCoordinate {
    /// Type name: `Post`
    Type(String),
    /// Type + field (or enum value): `User.posts`, `Status.ACTIVE`
    Field {
        type_name: String,
        field_name: String,
    },
    /// Field argument: `Query.user(id:)`
    FieldArgument {
        type_name: String,
        field_name: String,
        arg_name: String,
    },
    /// Directive: `@deprecated`
    Directive(String),
    /// Directive argument: `@deprecated(reason:)`
    DirectiveArgument {
        directive_name: String,
        arg_name: String,
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

        // Directive coordinates: @name or @name(arg:)
        if let Some(rest) = s.strip_prefix('@') {
            return Self::parse_directive(rest, s);
        }

        // Type.field or Type.field(arg:) or Type
        if let Some(dot_pos) = s.find('.') {
            let type_name = &s[..dot_pos];
            let after_dot = &s[dot_pos + 1..];
            if type_name.is_empty() || after_dot.is_empty() {
                return Err(SchemaError::InvalidCoordinate(format!(
                    "invalid coordinate '{s}': both type and field must be non-empty"
                )));
            }

            // Check for field argument: Type.field(arg:)
            if let Some(paren_pos) = after_dot.find('(') {
                return Self::parse_field_argument(type_name, after_dot, paren_pos, s);
            }

            Ok(SchemaCoordinate::Field {
                type_name: type_name.to_string(),
                field_name: after_dot.to_string(),
            })
        } else {
            Ok(SchemaCoordinate::Type(s.to_string()))
        }
    }

    fn parse_directive(rest: &str, original: &str) -> Result<Self, SchemaError> {
        if rest.is_empty() {
            return Err(SchemaError::InvalidCoordinate(format!(
                "invalid coordinate '{original}': directive name cannot be empty"
            )));
        }

        // @directive(arg:)
        if let Some(paren_pos) = rest.find('(') {
            let directive_name = &rest[..paren_pos];
            let inside = &rest[paren_pos + 1..];
            if directive_name.is_empty() {
                return Err(SchemaError::InvalidCoordinate(format!(
                    "invalid coordinate '{original}': directive name cannot be empty"
                )));
            }
            let arg_name = inside.strip_suffix(":)").ok_or_else(|| {
                SchemaError::InvalidCoordinate(format!(
                    "invalid coordinate '{original}': argument must end with ':)'"
                ))
            })?;
            if arg_name.is_empty() {
                return Err(SchemaError::InvalidCoordinate(format!(
                    "invalid coordinate '{original}': argument name cannot be empty"
                )));
            }
            Ok(SchemaCoordinate::DirectiveArgument {
                directive_name: directive_name.to_string(),
                arg_name: arg_name.to_string(),
            })
        } else {
            Ok(SchemaCoordinate::Directive(rest.to_string()))
        }
    }

    fn parse_field_argument(
        type_name: &str,
        after_dot: &str,
        paren_pos: usize,
        original: &str,
    ) -> Result<Self, SchemaError> {
        let field_name = &after_dot[..paren_pos];
        let inside = &after_dot[paren_pos + 1..];
        if field_name.is_empty() {
            return Err(SchemaError::InvalidCoordinate(format!(
                "invalid coordinate '{original}': field name cannot be empty"
            )));
        }
        let arg_name = inside.strip_suffix(":)").ok_or_else(|| {
            SchemaError::InvalidCoordinate(format!(
                "invalid coordinate '{original}': argument must end with ':)'"
            ))
        })?;
        if arg_name.is_empty() {
            return Err(SchemaError::InvalidCoordinate(format!(
                "invalid coordinate '{original}': argument name cannot be empty"
            )));
        }
        Ok(SchemaCoordinate::FieldArgument {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
            arg_name: arg_name.to_string(),
        })
    }

    /// Returns the type name for type-based coordinates, or `None` for directives.
    pub fn type_name(&self) -> Option<&str> {
        match self {
            SchemaCoordinate::Type(name) => Some(name),
            SchemaCoordinate::Field { type_name, .. }
            | SchemaCoordinate::FieldArgument { type_name, .. } => Some(type_name),
            SchemaCoordinate::Directive(_) | SchemaCoordinate::DirectiveArgument { .. } => None,
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
            SchemaCoordinate::FieldArgument {
                type_name,
                field_name,
                arg_name,
            } => write!(f, "{type_name}.{field_name}({arg_name}:)"),
            SchemaCoordinate::Directive(name) => write!(f, "@{name}"),
            SchemaCoordinate::DirectiveArgument {
                directive_name,
                arg_name,
            } => write!(f, "@{directive_name}({arg_name}:)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Existing variants ---

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

    // --- Field argument ---

    #[test]
    fn parse_field_argument() {
        let coord = SchemaCoordinate::parse("Query.user(id:)").unwrap();
        assert_eq!(
            coord,
            SchemaCoordinate::FieldArgument {
                type_name: "Query".into(),
                field_name: "user".into(),
                arg_name: "id".into(),
            }
        );
    }

    #[test]
    fn display_field_argument() {
        let coord = SchemaCoordinate::FieldArgument {
            type_name: "Query".into(),
            field_name: "user".into(),
            arg_name: "id".into(),
        };
        assert_eq!(coord.to_string(), "Query.user(id:)");
    }

    #[test]
    fn reject_field_argument_missing_colon_paren() {
        assert!(SchemaCoordinate::parse("Query.user(id)").is_err());
    }

    #[test]
    fn reject_field_argument_empty_arg() {
        assert!(SchemaCoordinate::parse("Query.user(:)").is_err());
    }

    #[test]
    fn reject_field_argument_empty_field() {
        assert!(SchemaCoordinate::parse("Query.(id:)").is_err());
    }

    // --- Directive ---

    #[test]
    fn parse_directive() {
        let coord = SchemaCoordinate::parse("@deprecated").unwrap();
        assert_eq!(coord, SchemaCoordinate::Directive("deprecated".into()));
    }

    #[test]
    fn display_directive() {
        let coord = SchemaCoordinate::Directive("deprecated".into());
        assert_eq!(coord.to_string(), "@deprecated");
    }

    #[test]
    fn reject_bare_at() {
        assert!(SchemaCoordinate::parse("@").is_err());
    }

    // --- Directive argument ---

    #[test]
    fn parse_directive_argument() {
        let coord = SchemaCoordinate::parse("@deprecated(reason:)").unwrap();
        assert_eq!(
            coord,
            SchemaCoordinate::DirectiveArgument {
                directive_name: "deprecated".into(),
                arg_name: "reason".into(),
            }
        );
    }

    #[test]
    fn display_directive_argument() {
        let coord = SchemaCoordinate::DirectiveArgument {
            directive_name: "deprecated".into(),
            arg_name: "reason".into(),
        };
        assert_eq!(coord.to_string(), "@deprecated(reason:)");
    }

    #[test]
    fn reject_directive_argument_missing_colon_paren() {
        assert!(SchemaCoordinate::parse("@deprecated(reason)").is_err());
    }

    #[test]
    fn reject_directive_argument_empty_arg() {
        assert!(SchemaCoordinate::parse("@deprecated(:)").is_err());
    }

    // --- type_name() ---

    #[test]
    fn type_name_for_type() {
        let coord = SchemaCoordinate::Type("Post".into());
        assert_eq!(coord.type_name(), Some("Post"));
    }

    #[test]
    fn type_name_for_field() {
        let coord = SchemaCoordinate::Field {
            type_name: "User".into(),
            field_name: "posts".into(),
        };
        assert_eq!(coord.type_name(), Some("User"));
    }

    #[test]
    fn type_name_for_field_argument() {
        let coord = SchemaCoordinate::FieldArgument {
            type_name: "Query".into(),
            field_name: "user".into(),
            arg_name: "id".into(),
        };
        assert_eq!(coord.type_name(), Some("Query"));
    }

    #[test]
    fn type_name_for_directive() {
        let coord = SchemaCoordinate::Directive("deprecated".into());
        assert_eq!(coord.type_name(), None);
    }

    #[test]
    fn type_name_for_directive_argument() {
        let coord = SchemaCoordinate::DirectiveArgument {
            directive_name: "deprecated".into(),
            arg_name: "reason".into(),
        };
        assert_eq!(coord.type_name(), None);
    }

    // --- Round-trip: parse → display → parse ---

    #[test]
    fn round_trip_all_variants() {
        let cases = [
            "Post",
            "User.posts",
            "Query.user(id:)",
            "@deprecated",
            "@deprecated(reason:)",
        ];
        for input in cases {
            let coord = SchemaCoordinate::parse(input).unwrap();
            let displayed = coord.to_string();
            let reparsed = SchemaCoordinate::parse(&displayed).unwrap();
            assert_eq!(coord, reparsed, "round-trip failed for '{input}'");
        }
    }
}
