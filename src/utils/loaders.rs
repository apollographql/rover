use crate::utils::parsers::SchemaLocation;
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;

/// this fn takes either a filepath (e.g. "./schema.graphql") or a `-`
/// indicating stdin and attempts to load sdl from one of those two locations
/// It can fail on loading the file or if stdin can't be read.
pub fn load_schema_from_flag(loc: &SchemaLocation) -> Result<String> {
    match loc {
        SchemaLocation::Stdin(stdin) => {
            let mut buffer = String::new();
            stdin.clone()
                .read_to_string(&mut buffer)
                .context("Failed while attempting to read SDL from stdin")?;
            // let mut buffer = String::new();
            // io::stdin()
            //     .read_to_string(&mut buffer)
            //     .context("Failed while loading from SDL file")?;
            Ok(buffer)
        }
        SchemaLocation::File(path) => {
            if Path::exists(&path) {
                let contents = std::fs::read_to_string(path)?;
                Ok(contents)
            } else {
                Err(anyhow::anyhow!(
                    "Invalid path. No file found at {}",
                    path.display()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{load_schema_from_flag, SchemaLocation};
    use assert_fs::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn load_schema_from_flag_loads() {
        let fixture = assert_fs::TempDir::new().unwrap();

        let test_file = fixture.child("schema.graphql");
        test_file
            .write_str("type Query { hello: String! }")
            .unwrap();

        let test_path = test_file.path().to_path_buf();
        let loc = SchemaLocation::File(test_path);

        let schema = load_schema_from_flag(&loc).unwrap();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn load_schema_from_flag_errs_on_bad_path() {
        let empty_path = "./wow.graphql";
        let loc = SchemaLocation::File(PathBuf::from(empty_path));

        let schema = load_schema_from_flag(&loc);
        assert_eq!(schema.is_err(), true);
    }

    // TODO: can we test stdin?
    // would that mean passing _actual_ stdin as a value in the enum?
}
