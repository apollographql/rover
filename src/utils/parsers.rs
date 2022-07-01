use camino::{Utf8Path, Utf8PathBuf};

use crate::{anyhow, error::RoverError, Context, Result, Suggestion};

use std::{
    fmt,
    io::{Error as IOError, ErrorKind as IOErrorKind, Read},
};

#[derive(Debug, PartialEq)]
pub enum FileDescriptorType {
    Stdin,
    File(Utf8PathBuf),
}

impl FileDescriptorType {
    pub fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> Result<String> {
        let buffer = match self {
            Self::Stdin => {
                let mut buffer = String::new();
                stdin
                    .read_to_string(&mut buffer)
                    .with_context(|| format!("Failed to read {} from stdin", file_description))?;
                Ok(buffer)
            }
            Self::File(file_path) => {
                if Utf8Path::exists(file_path) {
                    let contents = std::fs::read_to_string(file_path).with_context(|| {
                        format!("Could not read {} from {}", file_description, file_path)
                    })?;
                    Ok(contents)
                } else {
                    Err(RoverError::new(anyhow!(
                        "Invalid path. No file found at {}",
                        file_path
                    )))
                }
            }
        }?;
        if buffer.is_empty() || buffer == *"\n" || buffer == *"\r\n" {
            let mut err = RoverError::new(anyhow!("The {} you passed was empty", file_description));
            let suggestion = match self {
                Self::Stdin => {
                    "Make sure the command you are piping to Rover contains output.".to_string()
                }
                Self::File(config_path) => {
                    format!(
                        "'{}' exists, but contains nothing. Did you forget to save?",
                        config_path
                    )
                }
            };
            err.set_suggestion(Suggestion::Adhoc(suggestion));
            Err(err)
        } else {
            Ok(buffer)
        }
    }
}

impl fmt::Display for FileDescriptorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::File(config_path) => config_path.as_str(),
                Self::Stdin => "stdin",
            }
        )
    }
}

pub fn parse_file_descriptor(input: &str) -> std::result::Result<FileDescriptorType, IOError> {
    if input == "-" {
        Ok(FileDescriptorType::Stdin)
    } else if input.is_empty() {
        Err(IOError::new(
            IOErrorKind::InvalidInput,
            anyhow!("The file path you specified is an empty string, which is invalid."),
        ))
    } else {
        let path = Utf8PathBuf::from(input);
        Ok(FileDescriptorType::File(path))
    }
}

/// Parses a key:value pair from a string and returns a tuple of key:value.
/// If a full key:value can't be parsed, it will error.
pub fn parse_header(header: &str) -> std::result::Result<(String, String), IOError> {
    // only split once, a header's value may have a ":" in it, but not a key. Right?
    let pair: Vec<&str> = header.splitn(2, ':').collect();
    if pair.len() < 2 {
        let msg = format!("Could not parse \"key:value\" pair for provided header: \"{}\". Headers must be provided in key:value pairs, with quotes around the pair if there are any spaces in the key or value.", header);
        Err(IOError::new(IOErrorKind::InvalidInput, msg))
    } else {
        Ok((pair[0].to_string(), pair[1].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_file_descriptor, FileDescriptorType};
    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;
    use std::convert::TryFrom;

    #[test]
    fn it_correctly_parses_stdin_flag() {
        let fd = parse_file_descriptor("-").unwrap();

        match fd {
            FileDescriptorType::File(_) => panic!("parsed incorrectly as file"),
            _ => (),
        }
    }

    #[test]
    fn it_correctly_parses_path_option() {
        let fd = parse_file_descriptor("./schema.graphql").unwrap();
        match fd {
            FileDescriptorType::File(buf) => {
                assert_eq!(buf.to_string(), "./schema.graphql".to_string());
            }
            _ => panic!("parsed incorrectly as stdin"),
        }
    }

    #[test]
    fn it_errs_with_empty_path() {
        let fd = parse_file_descriptor("");
        assert!(fd.is_err());
    }

    #[test]
    fn load_schema_from_flag_loads() {
        let fixture = assert_fs::TempDir::new().unwrap();

        let test_file = fixture.child("schema.graphql");
        test_file
            .write_str("type Query { hello: String! }")
            .unwrap();

        let test_path = Utf8PathBuf::try_from(test_file.path().to_path_buf()).unwrap();
        let fd = FileDescriptorType::File(test_path);

        let schema = fd
            .read_file_descriptor("SDL", &mut "".to_string().as_bytes())
            .unwrap();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn load_schema_from_flag_errs_on_bad_path() {
        let empty_path = "./wow.graphql";
        let fd = FileDescriptorType::File(Utf8PathBuf::from(empty_path));

        let schema = fd.read_file_descriptor("SDL", &mut "".to_string().as_bytes());
        assert!(schema.is_err());
    }

    #[test]
    fn load_schema_from_stdin_works() {
        // input implements std::io::Read, so it should be a suitable
        // replacement for stdin
        let input = "type Query { hello: String! }".to_string();
        let fd = FileDescriptorType::Stdin;

        let schema = fd
            .read_file_descriptor("SDL", &mut input.as_bytes())
            .unwrap();
        assert_eq!(schema, std::str::from_utf8(input.as_ref()).unwrap());
    }

    #[test]
    fn empty_file_errors() {
        let input = "".to_string();
        let fd = FileDescriptorType::Stdin;

        let schema_result = fd.read_file_descriptor("SDL", &mut input.as_bytes());
        assert!(schema_result.is_err())
    }
}
