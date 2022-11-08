use camino::Utf8PathBuf;
use ci_info::types::Vendor as CiVendor;
use reqwest::blocking::Client;
use reqwest::Url;
use rover_client::shared::GitContext;
use semver::Version;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use wsl::is_wsl;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::env::{self, consts::OS};
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

    /// SHA-256 hash of the current working directory
    cwd_hash: String,

    /// SHA-256 hash of the git remote URL
    remote_url_hash: Option<String>,

    /// Information about the current architecture/platform
    platform: Platform,

    /// The current version of the CLI
    cli_version: Version,

    /// Where the telemetry data is being reported to
    #[serde(skip_serializing)]
    reporting_info: ReportingInfo,

    /// The reqwest Client sputnik uses to send telemetry data
    #[serde(skip_serializing)]
    client: Client,
}

/// Platform represents the platform the CLI is being run from
#[derive(Debug, Serialize)]
pub struct Platform {
    /// the platform from which the command was run (i.e. linux, macOS, windows or even wsl)
    os: String,

    /// if we think this command is being run in CI
    continuous_integration: Option<CiVendor>,
}

/// Command contains information about the command that was run
#[derive(Eq, PartialEq, Serialize, Clone, Debug)]
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
    is_telemetry_enabled: bool,
    endpoint: Url,
    user_agent: String,
}

impl Session {
    /// creates a new Session containing info about the current command
    /// being executed.
    pub fn new<T: Report>(app: &T) -> Result<Session, SputnikError> {
        let machine_id = app.machine_id()?;
        let command = app.serialize_command()?;
        let client = app.client();
        let reporting_info = ReportingInfo {
            is_telemetry_enabled: app.is_telemetry_enabled()?,
            endpoint: app.endpoint()?,
            user_agent: app.user_agent(),
        };
        let current_dir = Utf8PathBuf::try_from(env::current_dir()?)?;
        let session_id = Uuid::new_v4();
        let cwd_hash = get_cwd_hash(&current_dir);
        let remote_url_hash = get_repo_hash();

        let continuous_integration = if ci_info::is_ci() {
            ci_info::get().vendor
        } else {
            None
        };

        let os = if is_wsl() {
            "wsl".to_string()
        } else {
            OS.to_string()
        };

        let platform = Platform {
            os,
            continuous_integration,
        };

        let cli_version = Version::parse(app.version().as_str())?;

        Ok(Session {
            command,
            machine_id,
            session_id,
            cwd_hash,
            remote_url_hash,
            platform,
            cli_version,
            reporting_info,
            client,
        })
    }

    /// sends anonymous usage data to the endpoint defined in ReportingInfo.
    pub fn report(&self) -> Result<(), SputnikError> {
        if self.reporting_info.is_telemetry_enabled && !cfg!(debug_assertions) {
            // set timeout to 400 ms to prevent blocking for too long on reporting
            let timeout = Duration::from_millis(4000);
            let body = serde_json::to_string(&self)?;
            tracing::debug!("POSTing to {}", &self.reporting_info.endpoint);
            tracing::debug!("{}", body);
            self.client
                .post(self.reporting_info.endpoint.clone())
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
fn get_cwd_hash(current_dir: &Utf8PathBuf) -> String {
    format!("{:x}", Sha256::digest(current_dir.as_str().as_bytes()))
}

/// returns sha256 digest of the repository the tool was executed from.
fn get_repo_hash() -> Option<String> {
    GitContext::default()
        .remote_url
        .map(|remote_url| format!("{:x}", Sha256::digest(remote_url.as_bytes())))
}
