use anyhow::Result;
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
