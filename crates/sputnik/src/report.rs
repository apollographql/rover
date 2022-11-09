use camino::Utf8PathBuf;
use reqwest::blocking::Client;
use url::Url;
use uuid::Uuid;

use rover_std::Fs;

use crate::{Command, SputnikError};

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

    /// returns the Client to use when sending telemetry data
    fn client(&self) -> Client;
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
    Fs::write_file(path, &machine_str)?;
    Ok(machine_id)
}

#[cfg(test)]
mod tests {
    use super::{get_or_write_machine_id, write_machine_id};

    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;
    use std::convert::TryFrom;

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
