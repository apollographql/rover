use std::{collections::HashMap, convert::TryFrom, env, fmt::Debug, time::Duration};

use camino::Utf8PathBuf;
use ci_info::types::Vendor as CiVendor;
use reqwest::{Client, Url};
use rover_client::shared::GitContext;
use rover_std::sha256_hex;
use semver::Version;
use serde::Serialize;
use uuid::Uuid;
use wsl::is_wsl;

use crate::{Report, SputnikError};

/// The most that reporting telemetry may delay process exit, in total. This covers the entire
/// time to make the request, including name resolution and connecting, not just server latency.
const REPORT_TIMEOUT: Duration = Duration::from_millis(250);

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

    /// the cpu arch used
    arch: String,

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
        let client = app.client()?;
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
            env::consts::OS.to_string()
        };

        let platform = Platform {
            os,
            arch: env::consts::ARCH.to_string(),
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

    /// sends anonymous usage data to the endpoint defined in ReportingInfo, giving up after
    /// [`REPORT_TIMEOUT`] (250ms) however slow or unreachable the endpoint is.
    ///
    /// The budget is a plain `tokio::time::timeout` rather than `rover-http`'s `TimeoutLayer`,
    /// because sputnik sends with a bare `reqwest::Client`, not a Tower service. Moving it onto
    /// Tower is a separate refactor.
    pub async fn report(&self) -> Result<(), SputnikError> {
        tokio::time::timeout(REPORT_TIMEOUT, self.send())
            .await
            .unwrap_or(Err(SputnikError::TimedOut))
    }

    async fn send(&self) -> Result<(), SputnikError> {
        // TODO: consider whether we want to disable non-production telemetry or at least document
        //  the reasoning for not using it
        if cfg!(debug_assertions) {
            tracing::debug!("Skipping telemetry reporting");
            return Ok(());
        }
        if self.reporting_info.is_telemetry_enabled {
            let body = serde_json::to_string(&self)?;
            tracing::debug!("POSTing to {}", &self.reporting_info.endpoint);
            tracing::debug!("{}", body);
            self.client
                .post(self.reporting_info.endpoint.clone())
                .body(body)
                .header("User-Agent", &self.reporting_info.user_agent)
                .header("Content-Type", "application/json")
                .send()
                .await?;
        }

        Ok(())
    }
}

/// returns sha256 digest of the directory the tool was executed from.
fn get_cwd_hash(current_dir: &Utf8PathBuf) -> String {
    sha256_hex(current_dir.as_str())
}

/// returns sha256 digest of the repository the tool was executed from.
fn get_repo_hash() -> Option<String> {
    GitContext::default().remote_url.map(sha256_hex)
}

#[cfg(test)]
mod tests {
    use httpmock::{Method::POST, MockServer};
    use reqwest::Client;
    use rstest::*;
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn report_path() -> &'static str {
        "some/report"
    }

    #[fixture]
    fn user_agent() -> &'static str {
        "some-user-agent"
    }

    #[fixture]
    fn session() -> Session {
        let mut arguments = HashMap::new();
        arguments.insert("cowbell".into(), "--with-feeling".into());

        Session {
            command: Command {
                name: "test-command".to_string(),
                arguments,
            },
            machine_id: Uuid::max(),
            session_id: Uuid::nil(),
            cwd_hash: "current/working/directory".into(),
            remote_url_hash: None,
            platform: Platform {
                os: "rocks-and-lightning".into(),
                arch: "itecture".into(),
                continuous_integration: None,
            },
            cli_version: Version::parse("0.0.0-test").unwrap(),
            reporting_info: ReportingInfo {
                is_telemetry_enabled: true,
                endpoint: Url::parse(format!("http://0.0.0.0/{}", report_path()).as_str()).unwrap(),
                user_agent: user_agent().into(),
            },
            client: Client::new(),
        }
    }

    enum ReportCase {
        Success,
        TelemetryDisabled,
        TimedOut,
    }

    #[rstest]
    #[case::success(ReportCase::Success)]
    #[case::telemetry_disabled(ReportCase::TelemetryDisabled)]
    #[case::timedout(ReportCase::TimedOut)]
    #[tokio::test]
    async fn test_report(
        #[case] case: ReportCase,
        mut session: Session,
        report_path: &'static str,
        user_agent: &'static str,
    ) -> Result<(), anyhow::Error> {
        // Toggle between true/false for telemetry to test whether we fire requests when it's
        // disabled
        if let ReportCase::TelemetryDisabled = case {
            session.reporting_info.is_telemetry_enabled = false;
        } else {
            session.reporting_info.is_telemetry_enabled = true;
        }

        let server = MockServer::start();
        let addr = server.address().to_string();
        let mocked_addr = Url::parse(format!("http://{}/{}", addr, report_path).as_str()).unwrap();
        session.reporting_info.endpoint = mocked_addr;

        let mocked = server.mock(|when, then| {
            when.method(POST)
                // Annoyingly, the fixture needs to not have the preceding `/` for
                // Url::parse() because it doesn't strip them; so, don't remove this
                // preceding `/`
                .path(format!("/{}", report_path))
                .header("User-Agent", user_agent)
                .header("Content-Type", "application/json");

            match case {
                ReportCase::Success | ReportCase::TelemetryDisabled => then.status(200),
                ReportCase::TimedOut =>
                // This won't actually wait 10s over the timeout threshold; the timeout
                // will kick in and return an error
                {
                    then.status(504)
                        .delay(REPORT_TIMEOUT + Duration::from_secs(10))
                }
            };
        });

        let started = std::time::Instant::now();
        let res = session.report().await;
        let elapsed = started.elapsed();

        if let ReportCase::TelemetryDisabled = case {
            // When telemetry is disabled, we should expect not outbound calls
            mocked.assert_calls(0);
        } else {
            mocked.assert_calls(1);
        }

        let res = res.map_err(|error| error.to_string());
        match case {
            ReportCase::Success | ReportCase::TelemetryDisabled => {
                assert_that!(res).is_equal_to(Ok(()));
            }
            ReportCase::TimedOut => {
                assert_that!(res)
                    .is_equal_to(Err("Reporting anonymous usage data timed out.".to_string()));
                // The report gives up at the budget, not before and not long after.
                assert_that!(elapsed).is_greater_than_or_equal_to(REPORT_TIMEOUT);
                assert_that!(elapsed).is_less_than(REPORT_TIMEOUT + Duration::from_millis(500));
            }
        };

        Ok(())
    }
}
