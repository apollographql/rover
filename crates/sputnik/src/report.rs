use url::Url;
use uuid::Uuid;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Command, SputnikError};

/// Report defines the behavior of how anonymous usage data is reported.
pub trait Report {
    /// converts the struct to a json blob.
    fn serialize_command(&self) -> Result<Command, SputnikError>;

    /// checks if a user has enabled anonymous usage data.
    fn is_enabled(&self) -> bool;

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
    fn machine_id_config(&self) -> Result<PathBuf, SputnikError>;

    /// returns the globally persistent machine identifier
    /// and writes it if it does not exist
    /// the default implemenation uses self.machine_id_config()
    /// as the location the machine identifier is written to.
    fn machine_id(&self) -> Result<Uuid, SputnikError> {
        let config_path = self.machine_id_config()?;
        if Path::exists(&config_path) {
            if let Ok(contents) = fs::read_to_string(&config_path) {
                if let Ok(machine_uuid) = Uuid::parse_str(&contents) {
                    return Ok(machine_uuid);
                }
            }
        }

        write_machine_id(&config_path)
    }
}

fn write_machine_id(path: &PathBuf) -> Result<Uuid, SputnikError> {
    let machine_id = Uuid::new_v4();
    let mut file = File::create(path)?;
    file.write_all(machine_id.to_string().as_bytes())?;
    Ok(machine_id)
}
