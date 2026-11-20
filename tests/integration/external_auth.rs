use std::{io::Write, path::PathBuf, time::Duration};

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use rstest::{fixture, rstest};
use serde_json::{Value, json};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};

const TOKEN: &str = "external-private-access-token";
const REPLACEMENT: &str = "rotated-private-access-token";

struct ExternalContext {
    directory: TempDir,
    token: PathBuf,
    home: PathBuf,
}

impl ExternalContext {
    fn publish_to(path: &std::path::Path, bytes: &[u8]) {
        let mut file = NamedTempFile::new_in(path.parent().unwrap()).unwrap();
        file.write_all(bytes).unwrap();
        std::fs::rename(file.into_temp_path(), path).unwrap();
    }

    fn publish(&self, bytes: &[u8]) {
        Self::publish_to(&self.token, bytes);
    }

    fn command(&self) -> Command {
        let mut command = Command::cargo_bin("rover").unwrap();
        command
            .current_dir(self.directory.path())
            .timeout(Duration::from_secs(15))
            .env_remove("APOLLO_KEY")
            .env_remove("APOLLO_CLIENT_ID")
            .env_remove("APOLLO_CLIENT_SECRET")
            .env_remove("APOLLO_ACCESS_TOKEN_FILE")
            .env("NO_COLOR", "1")
            .arg("--config-home")
            .arg(&self.home)
            .args([
                "--skip-update-check",
                "--telemetry-disabled",
                "--no-config-notices",
                "--client-timeout",
                "1",
                "--format",
                "json",
                "--log",
                "debug",
            ]);
        command
    }

    fn authenticated_command(&self, server: &MockServer) -> Command {
        let mut command = self.command();
        command
            .arg("--access-token-file")
            .arg(&self.token)
            .args(["--registry-url", &server.url("/graphql")]);
        command
    }

    fn assert_credential_store_untouched(&self) {
        assert_that!((
            self.home.join("profiles").exists(),
            self.home.join("credentials.json").exists(),
        ))
        .is_equal_to((false, false));
    }

    fn normalize_error(&self, mut value: Value) -> Value {
        if let Some(message) = value["error"]["message"].as_str() {
            value["error"]["message"] =
                Value::String(message.replace(self.token.to_str().unwrap(), "[TOKEN_FILE]"));
        }
        value
    }
}

#[fixture]
fn external_context() -> ExternalContext {
    let directory = tempfile::tempdir().unwrap();
    let context = ExternalContext {
        home: directory.path().join("home"),
        token: directory.path().join("access-token"),
        directory,
    };
    context.publish(TOKEN.as_bytes());
    context
}

fn identity_response() -> Value {
    json!({"data": {"me": {
        "__typename": "User", "id": "external-account", "asActor": {"type": "USER"}
    }}})
}

#[rstest]
#[case::auth_option("auth", false, false)]
#[case::config_option("config", false, false)]
#[case::auth_environment("auth", true, false)]
#[case::config_environment("config", true, false)]
#[case::auth_option_precedence("auth", false, true)]
#[case::config_option_precedence("config", false, true)]
fn external_identity_needs_neither_stored_credentials_nor_userinfo(
    external_context: ExternalContext,
    #[case] noun: &str,
    #[case] environment: bool,
    #[case] bad_environment: bool,
) {
    let server = MockServer::start();
    let platform = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", format!("Bearer {TOKEN}"))
            .header_missing("x-api-key")
            .body_includes("ConfigWhoAmIQuery");
        then.status(200).json_body(identity_response());
    });
    let oauth = server.mock(|when, then| {
        when.path("/userinfo");
        then.status(500);
    });
    let mut command = external_context.command();
    command
        .args(["--registry-url", &server.url("/graphql")])
        .args(["--oauth-whoami-url", &server.url("/userinfo")])
        .env("APOLLO_KEY", "");
    if environment {
        command.env("APOLLO_ACCESS_TOKEN_FILE", &external_context.token);
    } else {
        command
            .arg("--access-token-file")
            .arg(&external_context.token);
    }
    if bad_environment {
        command.env(
            "APOLLO_ACCESS_TOKEN_FILE",
            external_context.directory.path().join("missing"),
        );
    }

    let output = command.args([noun, "whoami"]).output().unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    insta::assert_json_snapshot!(
        if environment {
            "identity_environment"
        } else {
            "identity_option"
        },
        json,
    );
    assert_that!(String::from_utf8_lossy(&output.stderr).contains(TOKEN)).is_equal_to(false);
    assert_that!(std::fs::read(&external_context.token).unwrap())
        .is_equal_to(TOKEN.as_bytes().to_vec());
    platform.assert_calls(1);
    oauth.assert_calls(0);
    external_context.assert_credential_store_untouched();
}

#[rstest]
#[case::auth("auth")]
#[case::config("config")]
fn external_identity_unmasks_the_token_used_by_the_successful_attempt(
    external_context: ExternalContext,
    #[case] noun: &str,
) {
    let server = MockServer::start();
    let path = external_context.token.clone();
    let first = server.mock(|when, then| {
        when.path("/graphql")
            .header("authorization", format!("Bearer {TOKEN}"));
        then.respond_with(move |_| {
            ExternalContext::publish_to(&path, REPLACEMENT.as_bytes());
            http::Response::builder()
                .status(401)
                .body("")
                .unwrap()
                .into()
        });
    });
    let path = external_context.token.clone();
    let success = server.mock(|when, then| {
        when.path("/graphql")
            .header("authorization", format!("Bearer {REPLACEMENT}"))
            .header("tracestate", "rover=retries:1");
        then.respond_with(move |_| {
            ExternalContext::publish_to(&path, b"not-used-by-this-lookup");
            http::Response::builder()
                .status(200)
                .body(identity_response().to_string())
                .unwrap()
                .into()
        });
    });

    let output = external_context
        .authenticated_command(&server)
        .args([noun, "whoami", "--insecure-unmask-key"])
        .output()
        .unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    insta::assert_json_snapshot!(
        "identity_unmasked",
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    );
    assert_that!(String::from_utf8_lossy(&output.stderr).contains(REPLACEMENT)).is_equal_to(false);
    assert_that!(String::from_utf8_lossy(&output.stderr).contains(TOKEN)).is_equal_to(false);
    first.assert_calls(1);
    success.assert_calls(1);
    external_context.assert_credential_store_untouched();
}

#[rstest]
#[case::auth_missing("auth", None, "missing")]
#[case::config_missing("config", None, "missing")]
#[case::auth_invalid("auth", Some("Bearer private-file-contents"), "invalid")]
#[case::config_invalid("config", Some("Bearer private-file-contents"), "invalid")]
fn external_source_failures_keep_their_codes_and_private_contents(
    external_context: ExternalContext,
    #[case] noun: &str,
    #[case] contents: Option<&str>,
    #[case] snapshot: &str,
) {
    let server = MockServer::start();
    let unexpected = server.mock(|_, then| {
        then.status(500);
    });
    match contents {
        Some(contents) => external_context.publish(contents.as_bytes()),
        None => std::fs::remove_file(&external_context.token).unwrap(),
    }

    let output = external_context
        .authenticated_command(&server)
        .args([noun, "whoami"])
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    let json = serde_json::from_slice::<Value>(&output.stdout).unwrap();
    insta::assert_json_snapshot!(
        format!("source_{snapshot}"),
        external_context.normalize_error(json),
    );
    assert_that!(String::from_utf8_lossy(&output.stderr).contains("private-file-contents"))
        .is_equal_to(false);
    assert_that!(String::from_utf8_lossy(&output.stdout).contains("private-file-contents"))
        .is_equal_to(false);
    unexpected.assert_calls(0);
    external_context.assert_credential_store_untouched();
}

#[rstest]
#[case::key("APOLLO_KEY", "private-key", "key")]
#[case::client_id("APOLLO_CLIENT_ID", "private-client", "client_id")]
#[case::empty_id("APOLLO_CLIENT_ID", "", "client_id")]
#[case::secret("APOLLO_CLIENT_SECRET", "private-secret", "secret")]
#[case::empty_secret("APOLLO_CLIENT_SECRET", "", "secret")]
fn external_conflicts_fail_before_graphql_or_oauth(
    external_context: ExternalContext,
    #[case] name: &str,
    #[case] value: &str,
    #[case] snapshot: &str,
) {
    let server = MockServer::start();
    let unexpected = server.mock(|_, then| {
        then.status(500);
    });

    let output = external_context
        .authenticated_command(&server)
        .env(name, value)
        .args(["--oauth-token-url", &server.url("/token")])
        .args(["graph", "fetch", "example@current"])
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    insta::assert_json_snapshot!(
        format!("conflict_{snapshot}"),
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
    );
    if !value.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_that!(
            stderr
                .lines()
                .filter(|line| line.contains("conflicts with"))
                .any(|line| line.contains(value))
        )
        .is_equal_to(false);
        assert_that!(String::from_utf8_lossy(&output.stdout).contains(value)).is_equal_to(false);
    }
    unexpected.assert_calls(0);
    external_context.assert_credential_store_untouched();
}

#[rstest]
#[case::unauthorized(401, json!({"error": TOKEN}), "rejected")]
#[case::not_acceptable(406, json!({"error": TOKEN}), "rejected")]
#[case::forbidden(403, json!({"error": TOKEN}), "permission")]
#[case::body_rejection(200, json!({"data": null, "errors": [{
    "message": format!("Unauthorized: Invalid credentials provided: {TOKEN}")
}]}), "rejected")]
#[case::no_identity(200, json!({"data": {"me": null}}), "rejected")]
fn external_auth_rejections_and_permissions_have_distinct_safe_diagnostics(
    external_context: ExternalContext,
    #[case] status: u16,
    #[case] body: Value,
    #[case] snapshot: &str,
    #[values("auth", "config")] noun: &str,
    #[values(false, true)] unmask: bool,
) {
    let server = MockServer::start();
    let platform = server.mock(|when, then| {
        when.path("/graphql")
            .header("authorization", format!("Bearer {TOKEN}"));
        then.status(status).json_body(body);
    });

    let mut command = external_context.authenticated_command(&server);
    command.args([noun, "whoami"]);
    if unmask {
        command.arg("--insecure-unmask-key");
    }
    let output = command.output().unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    insta::assert_json_snapshot!(
        format!("failure_{snapshot}"),
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
    );
    assert_that!(String::from_utf8_lossy(&output.stderr).contains(TOKEN)).is_equal_to(false);
    assert_that!(String::from_utf8_lossy(&output.stdout).contains(TOKEN)).is_equal_to(false);
    platform.assert_calls(1);
}

#[rstest]
#[case::graph("graph")]
#[case::subgraph("subgraph")]
fn external_auth_is_not_used_for_introspection(
    external_context: ExternalContext,
    #[case] noun: &str,
) {
    let server = MockServer::start();
    let introspection = server.mock(|when, then| {
        when.method(POST)
            .header_missing("authorization")
            .header_missing("x-api-key");
        then.status(200).json_body(json!({"data": null}));
    });
    std::fs::remove_file(&external_context.token).unwrap();

    let output = external_context
        .authenticated_command(&server)
        .env("APOLLO_CLIENT_ID", "partial-client")
        .args([noun, "introspect", &server.url("/introspection")])
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    assert_that!(String::from_utf8_lossy(&output.stdout).contains("E067")).is_equal_to(false);
    assert_that!(String::from_utf8_lossy(&output.stderr).contains("conflicts with"))
        .is_equal_to(false);
    introspection.assert_calls(1);
}

#[rstest]
#[case::auth(&["auth", "whoami"][..])]
#[case::legacy(&["graph", "fetch", "example@current"][..])]
fn external_tokens_reflected_in_upstream_errors_are_redacted(
    external_context: ExternalContext,
    #[case] args: &[&str],
) {
    let server = MockServer::start();
    let platform = server.mock(|when, then| {
        when.path("/graphql")
            .header("authorization", format!("Bearer {TOKEN}"));
        then.status(200)
            .header("x-reflected-credential", TOKEN)
            .json_body(json!({"data": null, "errors": [{
                "message": format!("Operation failed for credential {TOKEN}"),
                "extensions": {"response": {"body": {"errors": [{
                    "message": format!("Upstream refused {TOKEN}")
                }]}}}
            }]}));
    });

    let output = external_context
        .authenticated_command(&server)
        .args(args)
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    assert_that!(String::from_utf8_lossy(&output.stdout).contains(TOKEN)).is_equal_to(false);
    assert_that!(String::from_utf8_lossy(&output.stderr).contains(TOKEN)).is_equal_to(false);
    platform.assert_calls(1);
}

#[rstest]
#[case::option(false)]
#[case::environment(true)]
fn an_empty_external_location_is_not_treated_as_absent(
    external_context: ExternalContext,
    #[case] environment: bool,
) {
    let server = MockServer::start();
    let unexpected = server.mock(|_, then| {
        then.status(500);
    });
    let mut command = external_context.command();
    command.args(["--registry-url", &server.url("/graphql")]);
    if environment {
        command.env("APOLLO_ACCESS_TOKEN_FILE", "");
    } else {
        command
            .args(["--access-token-file", ""])
            .env("APOLLO_ACCESS_TOKEN_FILE", &external_context.token);
    }

    let output = command.args(["auth", "whoami"]).output().unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_that!(json["error"]["code"].clone()).is_equal_to(json!("E066"));
    insta::assert_json_snapshot!(
        if environment {
            "empty_environment"
        } else {
            "empty_option"
        },
        json,
    );
    unexpected.assert_calls(0);
}

#[rstest]
#[case::list("list")]
#[case::show("show")]
fn local_commands_ignore_unavailable_conflicting_external_credentials(
    external_context: ExternalContext,
    #[case] command: &str,
) {
    std::fs::remove_file(&external_context.token).unwrap();

    let output = external_context
        .command()
        .arg("--access-token-file")
        .arg(&external_context.token)
        .env("APOLLO_KEY", "private-key")
        .env("APOLLO_CLIENT_ID", "")
        .args(["config", command])
        .output()
        .unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    assert_that!(String::from_utf8_lossy(&output.stderr).contains("conflicts with"))
        .is_equal_to(false);
}

#[rstest]
#[case::auth("auth")]
#[case::config("config")]
fn external_identity_ignores_stored_credentials_and_unused_oauth_settings(
    external_context: ExternalContext,
    #[case] noun: &str,
) {
    let config =
        houston::Config::new(Some(&external_context.home.to_str().unwrap()), None).unwrap();
    let profile = houston::Profile::new("default", &config);
    profile
        .set_setting("APOLLO_OAUTH_WHOAMI_URL", "not-a-url")
        .unwrap();
    let sensitive = external_context.home.join("profiles/default/.sensitive");
    std::fs::write(&sensitive, "api_key = \"stored-profile-key\"").unwrap();
    let before = std::fs::read(&sensitive).unwrap();
    let server = MockServer::start();
    let platform = server.mock(|when, then| {
        when.path("/graphql")
            .header("authorization", format!("Bearer {TOKEN}"))
            .header_missing("x-api-key");
        then.status(200).json_body(identity_response());
    });

    let output = external_context
        .authenticated_command(&server)
        .args([noun, "whoami"])
        .output()
        .unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    insta::assert_json_snapshot!(
        "identity_option",
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    );
    assert_that!(std::fs::read(&sensitive).unwrap()).is_equal_to(before);
    platform.assert_calls(1);
}

#[rstest]
fn external_auth_logout_manages_only_the_selected_profile_without_reading_the_source(
    external_context: ExternalContext,
) {
    let config =
        houston::Config::new(Some(&external_context.home.to_str().unwrap()), None).unwrap();
    let profile_name = format!(
        "external-auth-test-{}",
        external_context
            .directory
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
    );
    let profile_dir = config.home.join("profiles").join(&profile_name);
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
        profile_dir.join(".sensitive"),
        "access_token = \"stored-access-token\"\ngrant_type = \"device_code\"",
    )
    .unwrap();
    external_context.publish(b"Bearer unusable-external-source");
    let server = MockServer::start();
    let revocation = server.mock(|when, then| {
        when.method(POST)
            .path("/revoke")
            .body_includes("token=stored-access-token")
            .header_missing("authorization");
        then.status(200);
    });
    let external = server.mock(|when, then| {
        when.body_includes(TOKEN);
        then.status(500);
    });

    let output = external_context
        .authenticated_command(&server)
        .args([
            "--profile",
            &profile_name,
            "--oauth-revocation-url",
            &server.url("/revoke"),
        ])
        .env("APOLLO_KEY", "conflicting-key")
        .env("APOLLO_CLIENT_SECRET", "")
        .args(["auth", "logout"])
        .output()
        .unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    assert_that!(String::from_utf8_lossy(&output.stderr).contains("conflicts with"))
        .is_equal_to(false);
    assert_that!(std::fs::read(&external_context.token).unwrap())
        .is_equal_to(b"Bearer unusable-external-source".to_vec());
    assert_that!(
        external_context
            .home
            .join("profiles")
            .join(&profile_name)
            .join(".sensitive")
            .exists()
    )
    .is_equal_to(false);
    // OAuth revocation refuses plain HTTP; local deletion is still best effort.
    revocation.assert_calls(0);
    external.assert_calls(0);
}

#[rstest]
fn external_auth_login_still_saves_the_selected_profile_without_reading_the_source(
    external_context: ExternalContext,
) {
    let config =
        houston::Config::new(Some(&external_context.home.to_str().unwrap()), None).unwrap();
    let profile_name = format!(
        "external-login-test-{}",
        external_context
            .directory
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let profile = houston::Profile::new(&profile_name, &config);
    profile.set_setting("APOLLO_CLIENT_TIMEOUT", "7").unwrap();
    external_context.publish(b"Bearer unusable-external-source");
    let server = MockServer::start();
    let device = server.mock(|when, then| {
        when.method(POST)
            .path("/device")
            .header_missing("authorization");
        then.status(200).json_body(json!({
            "device_code": "device-code",
            "user_code": "display-code",
            "verification_uri": server.url("/verify"),
            "expires_in": 300,
            "interval": 0,
        }));
    });
    let token = server.mock(|when, then| {
        when.method(POST)
            .path("/token")
            .body_includes("device_code=device-code");
        then.status(200).json_body(json!({
            "access_token": "new-profile-access-token", "token_type": "Bearer",
        }));
    });

    let output = external_context
        .authenticated_command(&server)
        .args([
            "--profile",
            &profile_name,
            "--oauth-device-authorization-url",
            &server.url("/device"),
            "--oauth-token-url",
            &server.url("/token"),
            "auth",
            "login",
            "--no-browser",
        ])
        .env("APOLLO_KEY", "conflicting-key")
        .env("APOLLO_CLIENT_SECRET", "")
        .output()
        .unwrap();
    let session = profile
        .get_oauth_session()
        .map(|session| session.map(|session| session.access_token));
    // The generated profile name is unique, including for native secret-store backends.
    profile.delete_credential().unwrap();

    assert_that!(output.status.code())
        .named(&String::from_utf8_lossy(&output.stderr))
        .is_equal_to(Some(0));
    assert_that!(session)
        .is_ok()
        .is_equal_to(Some("new-profile-access-token".to_string()));
    assert_that!(profile.get_setting("APOLLO_CLIENT_TIMEOUT"))
        .is_ok()
        .is_equal_to(Some("7".to_string()));
    assert_that!(std::fs::read(&external_context.token).unwrap())
        .is_equal_to(b"Bearer unusable-external-source".to_vec());
    assert_that!(String::from_utf8_lossy(&output.stderr).contains("conflicts with"))
        .is_equal_to(false);
    device.assert_calls(1);
    token.assert_calls(1);
}
