use camino::Utf8PathBuf;
use reqwest::Client;
use rover_std::Fs;
use url::Url;
use uuid::Uuid;

use crate::{Command, InstallSalt, SputnikError};

/// The file the install salt is stored in, beside the machine identifier.
const INSTALL_SALT_FILE: &str = "install_salt.txt";

/// Report defines the behavior of how anonymous usage data is reported.
pub trait Report {
    /// converts the struct to a json blob.
    fn serialize_command(&self) -> Result<Command, SputnikError>;

    /// checks if a user has enabled anonymous usage data.
    fn is_telemetry_enabled(&self) -> Result<bool, SputnikError>;

    /// returns the endpoint that the data should be posted to.
    fn endpoint(&self) -> Result<Url, SputnikError>;

    /// returns the name of the tool, this is used to construct
    /// the User-Agent header.
    fn tool_name(&self) -> String;

    /// returns the version of the tool, this is used to construct
    /// the User-Agent header
    fn version(&self) -> String;

    /// constructs a user agent for the tool. by default, it calls
    /// self.tool_name() and self.version() to construct this.
    fn user_agent(&self) -> String {
        format!("{}/{}", self.tool_name(), self.version())
    }

    /// returns the location the tool stores a globally persistent
    /// machine identifier
    fn machine_id_config(&self) -> Result<Utf8PathBuf, SputnikError>;

    /// returns the globally persistent machine identifier
    /// and writes it if it does not exist
    /// the default implementation uses self.machine_id_config()
    /// as the location the machine identifier is written to.
    fn machine_id(&self) -> Result<Uuid, SputnikError> {
        let config_path = self.machine_id_config()?;
        get_or_write_machine_id(&config_path)
    }

    /// returns the install salt, which is stored beside the machine identifier and generated
    /// there if it does not exist. It is never sent.
    fn install_salt(&self) -> Result<InstallSalt, SputnikError> {
        let path = self.machine_id_config()?.with_file_name(INSTALL_SALT_FILE);
        InstallSalt::read_or_create(&path)
    }

    /// returns the Client to use when sending telemetry data
    fn client(&self) -> Result<Client, SputnikError>;
}

fn get_or_write_machine_id(path: &Utf8PathBuf) -> Result<Uuid, SputnikError> {
    if let Ok(contents) = Fs::read_file(path) {
        if let Ok(machine_uuid) = Uuid::parse_str(contents.trim()) {
            return Ok(machine_uuid);
        }
    }
    write_machine_id(path)
}

fn write_machine_id(path: &Utf8PathBuf) -> Result<Uuid, SputnikError> {
    let machine_id = Uuid::new_v4();
    let machine_str = machine_id.to_string();
    Fs::write_file(path, machine_str)?;
    Ok(machine_id)
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;
    use reqwest::Client;
    use speculoos::prelude::*;
    use url::Url;

    use super::{get_or_write_machine_id, write_machine_id, Report, INSTALL_SALT_FILE};
    use crate::{Command, SputnikError};

    /// A tool that keeps its machine identifier at a given path, and does nothing else.
    struct Tool {
        machine_id_path: Utf8PathBuf,
    }

    impl Report for Tool {
        fn serialize_command(&self) -> Result<Command, SputnikError> {
            Err(SputnikError::CommandParseError)
        }

        fn is_telemetry_enabled(&self) -> Result<bool, SputnikError> {
            Ok(false)
        }

        fn endpoint(&self) -> Result<Url, SputnikError> {
            Ok(Url::parse("http://localhost")?)
        }

        fn tool_name(&self) -> String {
            "tool".to_string()
        }

        fn version(&self) -> String {
            "0.0.0".to_string()
        }

        fn machine_id_config(&self) -> Result<Utf8PathBuf, SputnikError> {
            Ok(self.machine_id_path.clone())
        }

        fn client(&self) -> Result<Client, SputnikError> {
            Ok(Client::new())
        }
    }

    /// the install salt lives in the same directory as the machine identifier
    #[test]
    fn the_install_salt_is_stored_beside_the_machine_id() {
        let fixture = assert_fs::TempDir::new().unwrap();
        let tool = Tool {
            machine_id_path: Utf8PathBuf::try_from(
                fixture.child("machine.txt").path().to_path_buf(),
            )
            .unwrap(),
        };

        let salt = tool.install_salt().unwrap();

        assert_that!(std::fs::read_to_string(
            fixture.child(INSTALL_SALT_FILE).path()
        ))
        .is_ok()
        .is_equal_to(salt.to_hex());
        assert_that!(tool.install_salt()).is_ok().is_equal_to(salt);
    }

    /// if a machine ID hasn't been written already, one will be created
    /// and saved.
    #[test]
    fn it_can_write_machine_id() {
        let fixture = assert_fs::TempDir::new().unwrap();
        let test_file = fixture.child("test_write_machine_id.txt");
        let test_path = Utf8PathBuf::try_from(test_file.path().to_path_buf()).unwrap();
        assert!(write_machine_id(&test_path).is_ok());
    }

    /// write a machine ID to a file, and expect `get_or_write_machine_id`
    /// to retrieve it
    #[test]
    fn it_can_read_machine_id() {
        let fixture = assert_fs::TempDir::new().unwrap();
        let test_file = fixture.child("test_read_machine_id.txt");
        let test_path = Utf8PathBuf::try_from(test_file.path().to_path_buf()).unwrap();
        let written_uuid = write_machine_id(&test_path).expect("could not write machine id");
        let read_uuid = get_or_write_machine_id(&test_path).expect("could not read machine id");
        assert_eq!(written_uuid, read_uuid);
    }

    /// try to read a machine ID that does not yet exist
    /// and ensure that it creates and saves a new one
    /// before retrieving
    #[test]
    fn it_can_read_and_write_machine_id() {
        let fixture = assert_fs::TempDir::new().unwrap();
        let test_file = fixture.child("test_read_and_write_machine_id.txt");
        let test_path = Utf8PathBuf::try_from(test_file.path().to_path_buf()).unwrap();
        assert!(get_or_write_machine_id(&test_path).is_ok());
    }
}
