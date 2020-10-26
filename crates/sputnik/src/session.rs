use ci_info::types::Vendor as CiVendor;
use reqwest::Url;
use semver::Version;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use std::collections::HashMap;
use std::env::{consts::OS, current_dir};
use std::fmt::Debug;
use std::time::Duration;

use crate::{Report, SputnikError};

/// The Session represents a usage of the CLI analogous to a web session
/// It contains the "url" (command path + flags) but doesn't contain any
/// values entered by the user. It also contains some identity information
/// for the user
#[derive(Debug, Serialize)]
pub struct Session {
    /// the command usage where information about the command is collected
    command: Command,

    /// Apollo generated machine ID. Stored globally at ~/.apollo/config.toml
    machine_id: Uuid,

    /// A unique session id
    session_id: Uuid,

    /// Directory hash. A hash of the current working directory
    cwd_hash: String,

    /// Information about the current architecture/platform
    platform: Platform,

    /// The current version of the CLI
    cli_version: Version,

    /// Where the telemetry data is being reported to
    #[serde(skip_serializing)]
    reporting_info: ReportingInfo,
}

/// Platform represents the platform the CLI is being run from
#[derive(Debug, Serialize)]
pub struct Platform {
    /// the platform from which the command was run (i.e. linux, macOS, or windows)
    os: String,

    /// if we think this command is being run in CI
    continuous_integration: Option<CiVendor>,
}

/// Command contains information about the command that was run
#[derive(PartialEq, Serialize, Clone, Debug)]
pub struct Command {
    /// the name of the command that was run.
    pub name: String,

    /// the arguments that were run with the command.
    pub arguments: HashMap<String, serde_json::Value>,
}

/// ReportingInfo represents information about where the telemetry data
/// should be reported to
#[derive(Debug)]
struct ReportingInfo {
    is_enabled: bool,
    endpoint: Url,
    user_agent: String,
}

impl Session {
    /// creates a new Session containing info about the current command
    /// being executed.
    pub fn new<T: Report>(app: &T) -> Result<Session, SputnikError> {
        let machine_id = app.machine_id()?;
        let command = app.serialize_command()?;
        let reporting_info = ReportingInfo {
            is_enabled: app.is_enabled(),
            endpoint: app.endpoint()?,
            user_agent: app.user_agent(),
        };
        let session_id = Uuid::new_v4();
        let cwd_hash = get_cwd_hash()?;

        let continuous_integration = if ci_info::is_ci() {
            ci_info::get().vendor
        } else {
            None
        };

        let platform = Platform {
            os: OS.to_string(),
            continuous_integration,
        };

        let cli_version = Version::parse(app.version().as_str())?;

        Ok(Session {
            command,
            machine_id,
            session_id,
            cwd_hash,
            platform,
            cli_version,
            reporting_info,
        })
    }

    /// sends anonymous usage data to the endpoint defined in ReportingInfo.
    pub fn report(&self) -> Result<(), SputnikError> {
        if self.reporting_info.is_enabled {
            // set timeout to 400 ms to prevent blocking for too long on reporting
            let timeout = Duration::from_millis(4000);
            let body = serde_json::to_string(&self)?;
            reqwest::blocking::Client::new()
                .post(self.reporting_info.endpoint.to_owned())
                .body(body)
                .header("User-Agent", &self.reporting_info.user_agent)
                .header("Content-Type", "application/json")
                .timeout(timeout)
                .send()?;
        }
        Ok(())
    }
}

/// returns sha256 digest of the directory the tool was executed from.
fn get_cwd_hash() -> Result<String, SputnikError> {
    let current_dir = current_dir()?;
    let current_dir_string = current_dir.to_string_lossy();
    let current_dir_bytes = current_dir_string.as_bytes();

    Ok(format!("{:x}", Sha256::digest(current_dir_bytes)))
}
