use std::fmt::{self, Display};

#[derive(Debug, PartialEq, Clone)]
/// Convenience enum to create a Description. Can be a `Top` level, a `Field`
/// level or an `Input` level. The variants are distinguished by the way they
/// get displayed, e.g. number of leading spaces.
pub enum Description {
    /// Top-level description.
    Top {
        /// Description.
        source: Option<String>,
    },
    /// Field-level description.
    /// This description gets additional leading spaces.
    Field {
        /// Description.
        source: Option<String>,
    },
    /// Input-level description.
    /// This description get an additional space at the end.
    Input {
        /// Description.
        source: Option<String>,
    },
}

impl Display for Description {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Description::Top { source } => {
                if let Some(description) = source {
                    if is_block_string_character(description) {
                        writeln!(f, "\"\"\"\n{}\n\"\"\"", description)?
                    } else {
                        writeln!(f, "\"\"\"{}\"\"\"", description)?
                    }
                }
            }
            Description::Field { source } => {
                if let Some(description) = source {
                    if is_block_string_character(description) {
                        writeln!(f, "  \"\"\"\n  {}\n  \"\"\"", description)?
                    } else {
                        writeln!(f, "  \"\"\"{}\"\"\"", description)?
                    }
                }
            }
            Description::Input { source } => {
                if let Some(description) = source {
                    if is_block_string_character(description) {
                        write!(f, "\"\"\"\n{}\n\"\"\" ", description)?
                    } else {
                        write!(f, "\"\"\"{}\"\"\" ", description)?
                    }
                }
            }
        }
        write!(f, "")
    }
}

fn is_block_string_character(s: &str) -> bool {
    s.contains('\n') || s.contains('"') || s.contains('\r')
}
#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_description_without_block_string_character() {
        let desc = Description::Top {
            source: Some(
                "Favourite cat nap spots include: plant corner, pile of clothes.".to_string(),
            ),
        };

        assert_eq!(
            desc.to_string(),
            r#""""Favourite cat nap spots include: plant corner, pile of clothes."""
"#
        );
    }

    #[test]
    fn it_encodes_description_with_quotations() {
        let desc = Description::Top {
            source: Some(
                "Favourite \"cat\" nap spots include: plant corner, pile of clothes.".to_string(),
            ),
        };

        assert_eq!(
            desc.to_string(),
            r#""""
Favourite "cat" nap spots include: plant corner, pile of clothes.
"""
"#
        );
    }

    #[test]
    fn it_encodes_description_with_new_line() {
        let desc = Description::Top {
            source: Some(
                "Favourite cat nap spots include:\nplant corner, pile of clothes.".to_string(),
            ),
        };

        assert_eq!(
            desc.to_string(),
            r#""""
Favourite cat nap spots include:
plant corner, pile of clothes.
"""
"#
        );
    }

    #[test]
    fn it_encodes_description_with_carriage_return() {
        let desc = Description::Top {
            source: Some(
                "Favourite cat nap spots include:\rplant corner,\rpile of clothes.".to_string(),
            ),
        };

        assert_eq!(
            desc.to_string(),
            String::from(
                "\"\"\"\nFavourite cat nap spots include:\rplant corner,\rpile of clothes.\n\"\"\"\n"
            )
        );
    }
}
