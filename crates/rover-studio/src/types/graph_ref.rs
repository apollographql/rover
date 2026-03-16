use std::{borrow::Cow, fmt, str::FromStr};

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Error resulting from the invalid construction of a GraphRef
#[derive(thiserror::Error, Debug)]
#[error(
    "Graph IDs must be in the format <NAME> or <NAME>@<VARIANT>, where <NAME> can only contain letters, numbers, or the characters `-` or `_`, and must be 64 characters or less. <VARIANT> must be 64 characters or less."
)]
pub struct InvalidGraphRef;

/// Represents a GraphOS GraphRef
#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq, derive_getters::Getters)]
pub struct GraphRef {
    name: Cow<'static, str>,
    variant: Cow<'static, str>,
}

impl GraphRef {
    /// Creates a new GraphRef from graph_id and variant
    pub fn new(
        graph_id: impl Into<Cow<'static, str>>,
        variant: Option<impl Into<Cow<'static, str>>>,
    ) -> Result<Self, InvalidGraphRef> {
        let graph_id = graph_id.into();
        let s = match variant {
            Some(v) => format!("{}@{}", graph_id, v.into()),
            None => graph_id.into_owned(),
        };
        Self::from_str(&s)
    }

    /// Consumes the GraphRef and returns `(name, variant)` as owned Strings.
    pub fn into_parts(self) -> (String, String) {
        (self.name.into_owned(), self.variant.into_owned())
    }
}

impl fmt::Display for GraphRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.variant)
    }
}

impl FromStr for GraphRef {
    type Err = InvalidGraphRef;

    /// NOTE: THIS IS A TEMPORARY SOLUTION. IN THE FUTURE, ALL GRAPH ID PARSING
    /// WILL HAPPEN IN THE BACKEND TO KEEP EVERYTHING CONSISTENT. THIS IS AN
    /// INCOMPLETE PLACEHOLDER, AND MAY NOT COVER EVERY SINGLE VALID USE CASE
    fn from_str(graph_id: &str) -> Result<Self, Self::Err> {
        let pattern = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}$").unwrap();
        let variant_pattern = Regex::new(r"^([a-zA-Z][a-zA-Z0-9_-]{0,63})@(.{0,63})$").unwrap();

        let valid_graph_name_only = pattern.is_match(graph_id);
        let valid_graph_with_variant = variant_pattern.is_match(graph_id);

        if valid_graph_name_only {
            Ok(GraphRef {
                name: Cow::Owned(graph_id.to_string()),
                variant: Cow::Borrowed("current"),
            })
        } else if valid_graph_with_variant {
            let matches = variant_pattern.captures(graph_id).unwrap();
            let name = matches.get(1).unwrap().as_str();
            let variant = matches.get(2).unwrap().as_str();
            Ok(GraphRef {
                name: Cow::Owned(name.to_string()),
                variant: Cow::Owned(variant.to_string()),
            })
        } else {
            Err(InvalidGraphRef)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::GraphRef;

    #[test]
    fn from_str_works() {
        assert!(GraphRef::from_str("engine#%^").is_err());
        assert!(
            GraphRef::from_str(
                "1234567890123456789012345678901234567890123456789012345678901234567890"
            )
            .is_err()
        );
        assert!(GraphRef::from_str("1boi").is_err());
        assert!(GraphRef::from_str("_eng").is_err());
        assert!(
            GraphRef::from_str(
                "engine@1234567890123456789012345678901234567890123456789012345678901234567890"
            )
            .is_err()
        );
        assert!(
            GraphRef::from_str(
                "engine1234567890123456789012345678901234567890123456789012345678901234567890@prod"
            )
            .is_err()
        );

        assert_eq!(
            GraphRef::from_str("engine@okay").unwrap(),
            GraphRef::new("engine", Some("okay")).unwrap()
        );
        assert_eq!(
            GraphRef::from_str("studio").unwrap(),
            GraphRef::new("studio", None::<&str>).unwrap()
        );
        assert_eq!(
            GraphRef::from_str("this_should_work").unwrap(),
            GraphRef::new("this_should_work", None::<&str>).unwrap()
        );
        assert_eq!(
            GraphRef::from_str("it-is-cool@my-special/variant:from$hell").unwrap(),
            GraphRef::new("it-is-cool", Some("my-special/variant:from$hell")).unwrap()
        );
    }

    #[test]
    fn new_accepts_static_str() {
        let g = GraphRef::new("mygraph", Some("current")).unwrap();
        assert_eq!(g.name(), "mygraph");
        assert_eq!(g.variant(), "current");
    }

    #[test]
    fn new_accepts_owned_string() {
        let name = "mygraph".to_string();
        let variant = "prod".to_string();
        let g = GraphRef::new(name, Some(variant)).unwrap();
        assert_eq!(g.name(), "mygraph");
        assert_eq!(g.variant(), "prod");
    }
}
