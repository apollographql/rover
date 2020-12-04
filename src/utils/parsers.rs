use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum SchemaLocation {
    Stdin,
    File(PathBuf),
}

// Stdin(Box<dyn std::io::Read>),
pub fn parse_schema_location(loc: &str) -> Result<SchemaLocation> {
    if loc == "-" {
        Ok(SchemaLocation::Stdin)
    } else if loc.is_empty() {
        Err(anyhow::anyhow!(
            "The path provided to find a schema is empty"
        ))
    } else {
        let path = PathBuf::from(loc);
        Ok(SchemaLocation::File(path))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_schema_location, SchemaLocation};

    #[test]
    fn it_correctly_parses_stdin_flag() {
        assert_eq!(parse_schema_location("-").unwrap(), SchemaLocation::Stdin);
    }

    #[test]
    fn it_correctly_parses_path_option() {
        let loc = parse_schema_location("./schema.graphql").unwrap();
        match loc {
            SchemaLocation::File(buf) => {
                assert_eq!(buf.to_str().unwrap(), "./schema.graphql");
            }
            _ => panic!("parsed incorrectly as stdin"),
        }
    }

    #[test]
    fn it_errs_with_empty_path() {
        let loc = parse_schema_location("");
        assert!(loc.is_err());
    }
}
