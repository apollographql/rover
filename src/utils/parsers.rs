use anyhow::Result;
use anyhow::*;
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum SchemaSource {
    Stdin,
    File(PathBuf),
}

pub fn parse_schema_source(loc: &str) -> Result<SchemaSource> {
    if loc == "-" {
        Ok(SchemaSource::Stdin)
    } else if loc.is_empty() {
        Err(anyhow::anyhow!(
            "The path provided to find a schema is empty"
        ))
    } else {
        let path = PathBuf::from(loc);
        Ok(SchemaSource::File(path))
    }
}

#[derive(Debug, Clone)]
pub struct GraphRef {
    pub name: String,
    pub variant: String,
}

/// NOTE: THIS IS A TEMPORARY SOLUTION. IN THE FUTURE, ALL GRAPH ID PARSING
/// WILL HAPPEN IN THE BACKEND TO KEEP EVERYTHING CONSISTENT. THIS IS AN
/// INCOMPLETE PLACEHOLDER, AND MAY NOT COVER EVERY SINGLE VALID USE CASE
pub fn parse_graph_ref(graph_id: &str) -> Result<GraphRef> {
    let pattern = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}$").unwrap();
    let variant_pattern = Regex::new(r"^([a-zA-Z][a-zA-Z0-9_-]{0,63})@(.{0,63})$").unwrap();

    let valid_graph_name_only = pattern.is_match(graph_id);
    let valid_graph_with_variant = variant_pattern.is_match(graph_id);

    if valid_graph_name_only {
        Ok(GraphRef {
            name: String::from(graph_id),
            variant: String::from("current"),
        })
    } else if valid_graph_with_variant {
        let matches = variant_pattern.captures(graph_id).unwrap();
        let name = matches.get(1).unwrap().as_str();
        let variant = matches.get(2).unwrap().as_str();
        Ok(GraphRef {
            name: String::from(name),
            variant: String::from(variant),
        })
    } else {
        Err(anyhow!("Graph IDs must be in the format <NAME> or <NAME>@<VARIANT>, where <NAME> can only contain letters, numbers, or the characters `-` or `_`, and must be 64 characters or less. <VARIANT> must al be 64 characters or less."))
    }
}

// #[test]
// fn parse_graph_id_works() {
//     assert!(parse_graph_id("engine#%^").is_err());
//     assert!(parse_graph_id("engine@okay").is_err());
//     assert!(parse_graph_id(
//         "1234567890123456789012345678901234567890123456789012345678901234567890"
//     )
//     .is_err());
//     assert!(parse_graph_id("1boi").is_err());
//     assert!(parse_graph_id("_eng").is_err());

//     assert_eq!("studio".to_string(), parse_graph_id("studio").unwrap());
//     assert_eq!(
//         "this_should_work".to_string(),
//         parse_graph_id("this_should_work").unwrap()
//     );
//     assert_eq!(
//         "it-is-cool".to_string(),
//         parse_graph_id("it-is-cool").unwrap()
//     );
// }

#[cfg(test)]
mod tests {
    use super::{parse_schema_source, SchemaSource};

    #[test]
    fn it_correctly_parses_stdin_flag() {
        assert_eq!(parse_schema_source("-").unwrap(), SchemaSource::Stdin);
    }

    #[test]
    fn it_correctly_parses_path_option() {
        let loc = parse_schema_source("./schema.graphql").unwrap();
        match loc {
            SchemaSource::File(buf) => {
                assert_eq!(buf.to_str().unwrap(), "./schema.graphql");
            }
            _ => panic!("parsed incorrectly as stdin"),
        }
    }

    #[test]
    fn it_errs_with_empty_path() {
        let loc = parse_schema_source("");
        assert!(loc.is_err());
    }
}
