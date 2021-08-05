use camino::Utf8PathBuf;

use crate::{error::RoverError, Result};

#[derive(Debug, PartialEq)]
pub enum SchemaSource {
    Stdin,
    File(Utf8PathBuf),
}

pub fn parse_schema_source(loc: &str) -> Result<SchemaSource> {
    if loc == "-" {
        Ok(SchemaSource::Stdin)
    } else if loc.is_empty() {
        Err(RoverError::parse_error(
            "The path provided to find a schema is empty",
        ))
    } else {
        let path = Utf8PathBuf::from(loc);
        Ok(SchemaSource::File(path))
    }
}

pub fn parse_query_count_threshold(threshold: &str) -> Result<i64> {
    let threshold = threshold.parse::<i64>()?;
    if threshold < 1 {
        Err(RoverError::parse_error(
            "The number of queries must be a positive integer.",
        ))
    } else {
        Ok(threshold)
    }
}

pub fn parse_query_percentage_threshold(threshold: &str) -> Result<f64> {
    let threshold = threshold.parse::<i64>()?;
    if !(0..=100).contains(&threshold) {
        Err(RoverError::parse_error(
            "Valid numbers are in the range 0 <= x <= 100",
        ))
    } else {
        Ok((threshold / 100) as f64)
    }
}

/// Parses a key:value pair from a string and returns a tuple of key:value.
/// If a full key:value can't be parsed, it will error.
pub fn parse_header(header: &str) -> Result<(String, String)> {
    // only split once, a header's value may have a ":" in it, but not a key. Right?
    let pair: Vec<&str> = header.splitn(2, ':').collect();
    if pair.len() < 2 {
        let msg = format!("Could not parse \"key:value\" pair for provided header: \"{}\". Headers must be provided in key:value pairs, with quotes around the pair if there are any spaces in the key or value.", header);
        Err(RoverError::parse_error(msg))
    } else {
        Ok((pair[0].to_string(), pair[1].to_string()))
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
                assert_eq!(buf.to_string(), "./schema.graphql".to_string());
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
