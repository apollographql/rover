use crate::utils::parsers::SchemaSource;
use crate::{anyhow, Context, Result};

use camino::Utf8Path;
use std::io::Read;

/// this fn takes 2 args: the first, an enum describing where to look to load
/// a schema - from stdin or a file's Utf8PathBuf, and the second, the reference to
/// stdin to load from, should it be needed.
pub fn load_schema_from_flag(loc: &SchemaSource, mut stdin: impl Read) -> Result<String> {
    match loc {
        SchemaSource::Stdin => {
            let mut buffer = String::new();
            stdin
                .read_to_string(&mut buffer)
                .context("Failed while attempting to read SDL from stdin")?;
            Ok(buffer)
        }
        SchemaSource::File(path) => {
            if Utf8Path::exists(&path) {
                let contents = std::fs::read_to_string(path)?;
                Ok(contents)
            } else {
                Err(anyhow!("Invalid path. No file found at {}", path).into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{load_schema_from_flag, SchemaSource};
    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;
    use std::convert::TryFrom;

    #[test]
    fn load_schema_from_flag_loads() {
        let fixture = assert_fs::TempDir::new().unwrap();

        let test_file = fixture.child("schema.graphql");
        test_file
            .write_str("type Query { hello: String! }")
            .unwrap();

        let test_path = Utf8PathBuf::try_from(test_file.path().to_path_buf()).unwrap();
        let loc = SchemaSource::File(test_path);

        let schema = load_schema_from_flag(&loc, std::io::stdin()).unwrap();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn load_schema_from_flag_errs_on_bad_path() {
        let empty_path = "./wow.graphql";
        let loc = SchemaSource::File(Utf8PathBuf::from(empty_path));

        let schema = load_schema_from_flag(&loc, std::io::stdin());
        assert_eq!(schema.is_err(), true);
    }

    #[test]
    fn load_schema_from_stdin_works() {
        // input implements std::io::Read, so it should be a suitable
        // replacement for stdin
        let input = b"type Query { hello: String! }";
        let loc = SchemaSource::Stdin;

        let schema = load_schema_from_flag(&loc, &input[..]).unwrap();
        assert_eq!(schema, std::str::from_utf8(input).unwrap());
    }
}
