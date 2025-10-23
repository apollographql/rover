use std::{fmt, io, str::FromStr};

use anyhow::{Context, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use rover_std::Fs;
use serde::Serialize;

use super::effect::read_stdin::ReadStdin;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum FileDescriptorType {
    Stdin,
    File(Utf8PathBuf),
}

impl FileDescriptorType {
    pub fn read_file_descriptor(
        &self,
        file_description: &str,
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<String> {
        let buffer = match self {
            Self::Stdin => {
                let buffer = read_stdin_impl
                    .read_stdin(file_description)
                    .with_context(|| format!("Failed to read {file_description} from stdin"))?;
                Ok(buffer)
            }
            Self::File(file_path) => {
                if Utf8Path::exists(file_path) {
                    let contents = Fs::read_file(file_path).with_context(|| {
                        format!("Could not read {file_description} from {file_path}")
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
                    format!("'{config_path}' exists, but contains nothing. Did you forget to save?")
                }
            };
            err.set_suggestion(RoverErrorSuggestion::Adhoc(suggestion));
            Err(err)
        } else {
            Ok(buffer)
        }
    }

    pub fn to_path_buf(&self) -> RoverResult<&Utf8PathBuf> {
        match &self {
            FileDescriptorType::Stdin => {
                Err(RoverError::new(anyhow!("Unable to get path buf for stdin")))
            }
            FileDescriptorType::File(file_path) => Ok(file_path),
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

impl FromStr for FileDescriptorType {
    type Err = io::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        if input == "-" {
            Ok(FileDescriptorType::Stdin)
        } else if input.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                anyhow!("The file path you specified is an empty string, which is invalid."),
            ))
        } else {
            let path = Utf8PathBuf::from(input);
            Ok(FileDescriptorType::File(path))
        }
    }
}

/// Parses a key:value pair from a string and returns a tuple of key:value.
/// If a full key:value can't be parsed, it will error.
pub fn parse_header(header: &str) -> std::result::Result<(String, String), io::Error> {
    // only split once, a header's value may have a ":" in it, but not a key. Right?
    let pair: Vec<&str> = header.splitn(2, ':').collect();
    if pair.len() < 2 {
        let msg = format!(
            "Could not parse \"key:value\" pair for provided header: \"{header}\". Headers must be provided in key:value pairs, with quotes around the pair if there are any spaces in the key or value."
        );
        Err(io::Error::new(io::ErrorKind::InvalidInput, msg))
    } else {
        Ok((pair[0].to_string(), pair[1].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, str::FromStr};

    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;
    use mockall::predicate;

    use super::FileDescriptorType;
    use crate::utils::effect::read_stdin::MockReadStdin;

    #[test]
    fn it_correctly_parses_stdin_flag() {
        let fd = FileDescriptorType::from_str("-").unwrap();
        if let FileDescriptorType::File(_) = fd {
            panic!("parsed incorrectly as file")
        }
    }

    #[test]
    fn it_correctly_parses_path_option() {
        let fd = FileDescriptorType::from_str("./schema.graphql").unwrap();
        match fd {
            FileDescriptorType::File(buf) => {
                assert_eq!(buf.to_string(), "./schema.graphql".to_string());
            }
            _ => panic!("parsed incorrectly as stdin"),
        }
    }

    #[test]
    fn it_errs_with_empty_path() {
        let fd = FileDescriptorType::from_str("");
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

        let mut mock_read_stdin = MockReadStdin::new();
        mock_read_stdin.expect_read_stdin().times(0);

        let schema = fd
            .read_file_descriptor("SDL", &mut mock_read_stdin)
            .unwrap();
        mock_read_stdin.checkpoint();
        assert_eq!(schema, "type Query { hello: String! }".to_string());
    }

    #[test]
    fn load_schema_from_flag_errs_on_bad_path() {
        let empty_path = "./wow.graphql";
        let fd = FileDescriptorType::File(Utf8PathBuf::from(empty_path));

        let mut mock_read_stdin = MockReadStdin::new();
        mock_read_stdin.expect_read_stdin().times(0);

        let schema = fd.read_file_descriptor("SDL", &mut mock_read_stdin);
        mock_read_stdin.checkpoint();
        assert!(schema.is_err());
    }

    #[test]
    fn load_schema_from_stdin_works() {
        let input = "type Query { hello: String! }".to_string();
        let fd = FileDescriptorType::Stdin;

        let mut mock_read_stdin = MockReadStdin::new();
        mock_read_stdin
            .expect_read_stdin()
            .times(1)
            .with(predicate::eq("SDL"))
            .returning({
                let input = input.to_string();
                move |_| Ok(input.to_string())
            });

        let schema = fd
            .read_file_descriptor("SDL", &mut mock_read_stdin)
            .unwrap();
        mock_read_stdin.checkpoint();
        assert_eq!(schema, std::str::from_utf8(input.as_ref()).unwrap());
    }

    #[test]
    fn empty_file_errors() {
        let fd = FileDescriptorType::Stdin;

        let mut mock_read_stdin = MockReadStdin::new();
        mock_read_stdin
            .expect_read_stdin()
            .times(1)
            .with(predicate::eq("SDL"))
            .returning(|_| Ok("".to_string()));

        let schema_result = fd.read_file_descriptor("SDL", &mut mock_read_stdin);
        mock_read_stdin.checkpoint();
        assert!(schema_result.is_err())
    }
}
