use std::fmt;
use std::str::FromStr;

use crate::RoverClientError;

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphRef {
    pub name: String,
    pub variant: String,
}

impl GraphRef {
    pub fn new(name: String, variant: Option<String>) -> Result<Self, RoverClientError> {
        let mut s = name;
        if let Some(variant) = variant {
            s.push('@');
            s.push_str(&variant);
        };
        Self::from_str(&s)
    }
}

impl fmt::Display for GraphRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.variant)
    }
}

impl FromStr for GraphRef {
    type Err = RoverClientError;

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
                name: graph_id.to_string(),
                variant: "current".to_string(),
            })
        } else if valid_graph_with_variant {
            let matches = variant_pattern.captures(graph_id).unwrap();
            let name = matches.get(1).unwrap().as_str();
            let variant = matches.get(2).unwrap().as_str();
            Ok(GraphRef {
                name: name.to_string(),
                variant: variant.to_string(),
            })
        } else {
            Err(RoverClientError::InvalidGraphRef)
        }
    }
}

impl<'de> Deserialize<'de> for GraphRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let graph_id = String::deserialize(deserializer)?;
        GraphRef::from_str(&graph_id).map_err(serde::de::Error::custom)
    }
}

impl Serialize for GraphRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::GraphRef;
    use std::str::FromStr;

    #[test]
    fn from_str_works() {
        assert!(GraphRef::from_str("engine#%^").is_err());
        assert!(GraphRef::from_str(
            "1234567890123456789012345678901234567890123456789012345678901234567890"
        )
        .is_err());
        assert!(GraphRef::from_str("1boi").is_err());
        assert!(GraphRef::from_str("_eng").is_err());
        assert!(GraphRef::from_str(
            "engine@1234567890123456789012345678901234567890123456789012345678901234567890"
        )
        .is_err());
        assert!(GraphRef::from_str(
            "engine1234567890123456789012345678901234567890123456789012345678901234567890@prod"
        )
        .is_err());

        assert_eq!(
            GraphRef::from_str("engine@okay").unwrap(),
            GraphRef {
                name: "engine".to_string(),
                variant: "okay".to_string()
            }
        );
        assert_eq!(
            GraphRef::from_str("studio").unwrap(),
            GraphRef {
                name: "studio".to_string(),
                variant: "current".to_string()
            }
        );
        assert_eq!(
            GraphRef::from_str("this_should_work").unwrap(),
            GraphRef {
                name: "this_should_work".to_string(),
                variant: "current".to_string()
            }
        );
        assert_eq!(
            GraphRef::from_str("it-is-cool@my-special/variant:from$hell").unwrap(),
            GraphRef {
                name: "it-is-cool".to_string(),
                variant: "my-special/variant:from$hell".to_string()
            }
        );
    }
}
