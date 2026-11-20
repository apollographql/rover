use std::{io::Write, path::PathBuf, time::Duration};

use camino::Utf8PathBuf;
use clap::{CommandFactory, FromArgMatches};
use httpmock::MockServer;
use rover_auth::external_token::ExternalTokenOrigin;
use rover_client::operations::config::who_am_i;
use rstest::{fixture, rstest};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};

use super::Rover;
use crate::{
    RoverError, RoverErrorCode,
    utils::{client::StudioClientConfig, env::RoverEnvKey},
};

struct ExternalContext {
    directory: TempDir,
    home: Utf8PathBuf,
    token: PathBuf,
}

impl ExternalContext {
    fn rover(&self, args: &[&str]) -> Rover {
        let mut all = vec![
            "rover",
            "--config-home",
            self.home.as_str(),
            "--skip-update-check",
            "--no-config-notices",
            "--telemetry-disabled",
        ];
        all.extend_from_slice(args);
        temp_env::with_var_unset("APOLLO_ACCESS_TOKEN_FILE", || {
            let matches = Rover::command().try_get_matches_from(all).unwrap();
            Rover::from_arg_matches(&matches)
                .unwrap()
                .with_arg_matches(matches)
        })
    }

    fn publish(&self, token: &[u8]) {
        let mut replacement = NamedTempFile::new_in(self.directory.path()).unwrap();
        replacement.write_all(token).unwrap();
        std::fs::rename(replacement.into_temp_path(), &self.token).unwrap();
    }
}

#[fixture]
fn external_context() -> ExternalContext {
    let directory = tempfile::tempdir().unwrap();
    ExternalContext {
        home: Utf8PathBuf::from_path_buf(directory.path().join("home")).unwrap(),
        token: directory.path().join("access-token"),
        directory,
    }
}

#[rstest]
#[case::key(RoverEnvKey::Key, "private-key", "APOLLO_KEY")]
#[case::client_id(RoverEnvKey::ClientId, "private-id", "APOLLO_CLIENT_ID")]
#[case::empty_client_id(RoverEnvKey::ClientId, "", "APOLLO_CLIENT_ID")]
#[case::client_secret(RoverEnvKey::ClientSecret, "private-secret", "APOLLO_CLIENT_SECRET")]
#[case::empty_client_secret(RoverEnvKey::ClientSecret, "", "APOLLO_CLIENT_SECRET")]
#[tokio::test]
async fn external_auth_conflicts_are_lazy_and_prevent_exchanges(
    external_context: ExternalContext,
    #[case] input: RoverEnvKey,
    #[case] value: &str,
    #[case] name: &str,
) {
    let server = MockServer::start_async().await;
    let unexpected = server.mock(|_, then| {
        then.status(500);
    });
    let mut rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "--oauth-token-url",
        &server.url("/token"),
        "graph",
        "fetch",
        "example@current",
    ]);
    rover.insert_env_var(input, value).unwrap();

    let config = rover.get_client_config().await.unwrap();
    let _unrelated_service = config.service().unwrap();
    let error = match config.get_authenticated_client(&rover.get_profile_opt()) {
        Ok(_) => panic!("conflicting authentication was accepted"),
        Err(error) => RoverError::new(error),
    };

    assert_that!(error.code()).is_equal_to(Some(RoverErrorCode::E079));
    assert_that!(serde_json::to_value(error).unwrap()).is_equal_to(serde_json::json!({
        "code": "E079",
        "message": format!(
            "--access-token-file conflicts with {name}. Remove the conflicting credential inputs to use the application-supplied token."
        ),
    }));
    unexpected.assert_calls(0);
}

#[rstest]
#[case::empty_key("", &["config", "list"][..])]
#[case::router_key("router-only-key", &["dev"][..])]
#[tokio::test]
async fn external_auth_allows_an_empty_key_and_the_dev_router_key(
    external_context: ExternalContext,
    #[case] key: &str,
    #[case] command: &[&str],
) {
    let mut args = vec![
        "--access-token-file",
        external_context.token.to_str().unwrap(),
    ];
    args.extend_from_slice(command);
    let mut rover = external_context.rover(&args);
    rover.insert_env_var(RoverEnvKey::Key, key).unwrap();
    let config = rover.get_client_config().await.unwrap();

    let client = config
        .get_authenticated_client(&rover.get_profile_opt())
        .unwrap();

    assert_that!(client.get_credential_origin())
        .is_equal_to(houston::CredentialOrigin::ExternalTokenFileOption);
    assert_that!(client.external_token_source().map(|source| source.origin()))
        .is_equal_to(Some(ExternalTokenOrigin::Option));
}

#[rstest]
#[case::list(&["config", "list"][..])]
#[case::show(&["config", "show"][..])]
#[tokio::test]
async fn external_auth_does_not_activate_on_local_commands(
    external_context: ExternalContext,
    #[case] command: &[&str],
) {
    let mut args = vec![
        "--access-token-file",
        external_context.token.to_str().unwrap(),
    ];
    args.extend_from_slice(command);
    let mut rover = external_context.rover(&args);
    rover
        .insert_env_var(RoverEnvKey::Key, "private-key")
        .unwrap();
    rover.insert_env_var(RoverEnvKey::ClientId, "").unwrap();

    let result = rover
        .execute_command()
        .await
        .map(|output| output.exit_code());

    assert_that!(result.map_err(|error| error.to_string()))
        .is_ok()
        .is_equal_to(0);
    assert_that!(rover.command.can_authenticate()).is_equal_to(false);
}

#[rstest]
#[tokio::test]
async fn external_auth_keeps_profile_settings_without_loading_the_profile_credential(
    external_context: ExternalContext,
) {
    let config = houston::Config::new(Some(&external_context.home), None).unwrap();
    houston::Profile::new("staging", &config)
        .set_setting("APOLLO_CLIENT_TIMEOUT", "7")
        .unwrap();
    let sensitive = external_context.home.join("profiles/staging/.sensitive");
    std::fs::write(&sensitive, "not valid credential TOML").unwrap();
    let rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "--profile",
        "staging",
        "config",
        "list",
    ]);

    let config = rover.get_client_config().await.unwrap();
    let client = config
        .get_authenticated_client(&rover.get_profile_opt())
        .unwrap();

    assert_that!(config.client_timeout().get_duration()).is_equal_to(Duration::from_secs(7));
    assert_that!(client.get_credential_origin())
        .is_equal_to(houston::CredentialOrigin::ExternalTokenFileOption);
    assert_that!(std::fs::read_to_string(sensitive).unwrap())
        .is_equal_to("not valid credential TOML".to_string());
}

#[rstest]
#[case::missing(None, RoverErrorCode::E067)]
#[case::invalid(Some(&b"Bearer private-file-contents"[..]), RoverErrorCode::E071)]
#[tokio::test]
async fn external_auth_source_failures_are_terminal_and_do_not_reach_the_network(
    external_context: ExternalContext,
    #[case] contents: Option<&[u8]>,
    #[case] code: RoverErrorCode,
) {
    if let Some(contents) = contents {
        external_context.publish(contents);
    }
    let server = MockServer::start_async().await;
    let unexpected = server.mock(|_, then| {
        then.status(200);
    });
    let rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "--registry-url",
        &server.url("/graphql"),
        "config",
        "list",
    ]);
    let config = rover.get_client_config().await.unwrap();
    let client = config
        .get_authenticated_client(&rover.get_profile_opt())
        .unwrap();

    let result = who_am_i::run(&client).await;
    let error = match result {
        Err(error) => RoverError::new(error),
        Ok(_) => panic!("a failed external source authenticated"),
    };

    assert_that!(error.code()).is_equal_to(Some(code));
    unexpected.assert_calls(0);
}

#[rstest]
#[case::absent(None, None, None)]
#[case::option(Some("flag-source"), None, Some(("flag-source", ExternalTokenOrigin::Option)))]
#[case::environment(None, Some("env-source"), Some(("env-source", ExternalTokenOrigin::Environment)))]
#[case::option_precedence(Some("flag-source"), Some("env-source"), Some(("flag-source", ExternalTokenOrigin::Option)))]
#[case::empty_option(Some(""), Some("env-source"), Some(("", ExternalTokenOrigin::Option)))]
#[case::empty_environment(None, Some(""), Some(("", ExternalTokenOrigin::Environment)))]
fn external_auth_selection_preserves_provenance_and_empty_locations(
    #[case] flag: Option<&str>,
    #[case] environment: Option<&str>,
    #[case] expected: Option<(&str, ExternalTokenOrigin)>,
) {
    let rover = temp_env::with_var("APOLLO_ACCESS_TOKEN_FILE", environment, || {
        let mut args = vec!["rover", "config", "list"];
        if let Some(flag) = flag {
            args.extend(["--access-token-file", flag]);
        }
        let matches = Rover::command().try_get_matches_from(args).unwrap();
        Rover::from_arg_matches(&matches)
            .unwrap()
            .with_arg_matches(matches)
    });
    let actual = rover
        .external_token_source()
        .map(|source| (source.path().to_path_buf(), source.origin()));
    let expected = expected.map(|(path, origin)| {
        (
            if path.is_empty() {
                PathBuf::new()
            } else {
                std::path::absolute(path).unwrap()
            },
            origin,
        )
    });

    assert_that!(actual).is_equal_to(expected);
}

#[rstest]
#[case::local(&["config", "list"][..], false)]
#[case::diagnostic(&["config", "show"][..], false)]
#[case::login(&["auth", "login"][..], false)]
#[case::logout(&["auth", "logout"][..], false)]
#[case::identity(&["auth", "whoami"][..], true)]
#[case::legacy_identity(&["config", "whoami"][..], true)]
#[case::graph(&["graph", "fetch", "example@current"][..], true)]
#[case::introspection(&["graph", "introspect", "http://localhost:1234"][..], false)]
#[case::subgraph_introspection(&["subgraph", "introspect", "http://localhost:1234"][..], false)]
#[case::dev(&["dev"][..], true)]
fn external_auth_warns_only_for_commands_that_can_authenticate(
    external_context: ExternalContext,
    #[case] args: &[&str],
    #[case] expected: bool,
) {
    assert_that!(external_context.rover(args).command.can_authenticate()).is_equal_to(expected);
}

#[rstest]
#[case::ordinary(&["config", "whoami"][..], true)]
#[case::dev(&["dev"][..], false)]
fn external_auth_warnings_name_inputs_but_not_values(
    external_context: ExternalContext,
    #[case] command: &[&str],
    #[case] expected_warning: bool,
) {
    let mut args = vec![
        "--access-token-file",
        external_context.token.to_str().unwrap(),
    ];
    args.extend_from_slice(command);
    let mut rover = external_context.rover(&args);
    rover
        .insert_env_var(RoverEnvKey::Key, "private-key")
        .unwrap();
    let capture = rover_print::print::testing::TerminalCapture::new(false);

    rover
        .external_authentication()
        .unwrap()
        .unwrap()
        .warn(&capture)
        .unwrap();

    let expected = if expected_warning {
        vec!["Warning: --access-token-file conflicts with APOLLO_KEY. Remove the conflicting credential inputs to use the application-supplied token. Authentication will fail when an authenticated operation starts.".to_string()]
    } else {
        vec![]
    };
    assert_that!(capture.lines()).is_equal_to(expected);
}

#[rstest]
#[case::plain(StudioClientConfig::service)]
#[case::download(StudioClientConfig::download_service)]
#[case::plugin_version(StudioClientConfig::plugin_version_service)]
#[tokio::test]
async fn external_auth_is_not_added_to_unrelated_http_services(
    external_context: ExternalContext,
    #[case] create_service: fn(&StudioClientConfig) -> anyhow::Result<rover_http::HttpService>,
) {
    use tower::ServiceExt;

    let server = MockServer::start_async().await;
    let unrelated = server.mock(|when, then| {
        when.header_missing("authorization")
            .header_missing("x-api-key");
        then.status(200);
    });
    let mut rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "config",
        "list",
    ]);
    rover
        .insert_env_var(RoverEnvKey::ClientSecret, "private-secret")
        .unwrap();
    let config = rover.get_client_config().await.unwrap();
    let service = create_service(&config).unwrap();
    let request = http::Request::get(server.url("/unrelated"))
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap();

    let response = service.oneshot(request).await.unwrap();

    assert_that!(response.status()).is_equal_to(http::StatusCode::OK);
    unrelated.assert_calls(1);
}

#[rstest]
#[tokio::test]
async fn external_identity_lookups_in_one_process_can_resolve_different_accounts(
    external_context: ExternalContext,
) {
    use crate::options::JsonOutput;

    let server = MockServer::start_async().await;
    let first = server.mock(|when, then| {
        when.header("authorization", "Bearer account-a-token");
        then.status(200)
            .json_body(serde_json::json!({"data": {"me": {
                "__typename": "User", "id": "account-a", "asActor": {"type": "USER"},
            }}}));
    });
    let second = server.mock(|when, then| {
        when.header("authorization", "Bearer account-b-token");
        then.status(200)
            .json_body(serde_json::json!({"data": {"me": {
                "__typename": "User", "id": "account-b", "asActor": {"type": "USER"},
            }}}));
    });
    let rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "--registry-url",
        &server.url("/graphql"),
        "auth",
        "whoami",
    ]);

    external_context.publish(b"account-a-token");
    let a = rover.execute_command().await.unwrap();
    external_context.publish(b"account-b-token");
    let b = rover.execute_command().await.unwrap();

    insta::assert_json_snapshot!(vec![JsonOutput::from(&a), JsonOutput::from(&b)]);
    first.assert_calls(1);
    second.assert_calls(1);
}

#[cfg(feature = "composition-js")]
#[rstest]
#[case::user("USER", None)]
#[case::service_account("SERVICE_ACCOUNT", Some(RoverErrorCode::E080))]
#[case::graph("GRAPH", Some(RoverErrorCode::E080))]
#[tokio::test]
async fn external_auth_init_uses_platform_identity_without_storing_a_credential(
    external_context: ExternalContext,
    #[case] actor: &str,
    #[case] code: Option<RoverErrorCode>,
) {
    use crate::command::init::states::UserAuthenticated;

    external_context.publish(b"application-token");
    let server = MockServer::start_async().await;
    let identity = server.mock(|when, then| {
        when.header("authorization", "Bearer application-token")
            .header_missing("x-api-key");
        then.status(200)
            .json_body(serde_json::json!({"data": {"me": {
                "__typename": "User", "id": "account-id", "asActor": {"type": actor},
            }}}));
    });
    let rover = external_context.rover(&[
        "--access-token-file",
        external_context.token.to_str().unwrap(),
        "--registry-url",
        &server.url("/graphql"),
        "init",
    ]);
    let config = rover.get_client_config().await.unwrap();

    let result = UserAuthenticated::new()
        .check_authentication(&config, &rover.get_profile_opt())
        .await;

    match code {
        Some(code) => {
            assert_that!(assert_that!(result).is_err().subject.code()).is_equal_to(Some(code))
        }
        None => assert_that!(result.map(|_| ()).map_err(|error| error.to_string()))
            .is_ok()
            .is_equal_to(()),
    }
    assert_that!(external_context.home.join("profiles").exists()).is_equal_to(false);
    identity.assert_calls(1);
}
