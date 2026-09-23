//! `rover install --plugin` reports each way a plugin can fail to install
//! under its own `error.code`, in the JSON envelope a script reads.

use assert_cmd::Command;
use httpmock::{Method, MockServer};
use insta::assert_json_snapshot;
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use crate::support::plugin_levels::{GlobalLevel, global_level};

/// How the plugin registry misbehaves.
#[derive(Clone, Copy, Debug)]
enum Registry {
    /// It can't resolve a floating request.
    CantResolve,
    /// It refuses the artifact with a status a retry won't change.
    RefusesTheArtifact,
    /// It serves an artifact that isn't a gzipped tarball.
    ServesAGarbledArtifact,
    /// It says the exact release was withdrawn, and names a newer one.
    WithdrewTheRelease,
    /// It never published the exact release.
    NeverPublishedTheRelease,
}

impl Registry {
    /// How many times Rover should ask it to resolve an alias, and how many
    /// times for the artifact. The code each case reports depends on this
    /// flow as much as on the answers.
    const fn expected_calls(self) -> (usize, usize) {
        match self {
            Self::CantResolve => (1, 0),
            // Only a withdrawal asks for the newest release to suggest.
            Self::WithdrewTheRelease => (1, 1),
            Self::RefusesTheArtifact
            | Self::ServesAGarbledArtifact
            | Self::NeverPublishedTheRelease => (0, 1),
        }
    }
}

fn install(level: &GlobalLevel, registry: Registry, request: &str) -> Value {
    let server = MockServer::start();
    let resolution = server.mock(|when, then| {
        when.method(Method::HEAD).path_includes("/latest-2");
        match registry {
            Registry::CantResolve => then.status(404),
            _ => then.status(302).header("X-Version", "v2.9.5"),
        };
    });
    let artifact = server.mock(|when, then| {
        when.method(Method::GET).path_includes("/tar/supergraph/");
        match registry {
            Registry::RefusesTheArtifact => then.status(403),
            Registry::ServesAGarbledArtifact => then.status(200).body("not a tarball"),
            Registry::WithdrewTheRelease => then.status(410),
            Registry::CantResolve | Registry::NeverPublishedTheRelease => then.status(404),
        };
    });
    let host = format!("http://{}", server.address());

    let output = Command::cargo_bin("rover")
        .unwrap()
        .args(["install", "--plugin", request])
        .args(["--client-timeout", "1", "--skip-update-check"])
        .args(["--format", "json"])
        .envs(level.env())
        .env("APOLLO_ROVER_DOWNLOAD_HOST", &host)
        .env("APOLLO_TELEMETRY_DISABLED", "true")
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_that!((output.status.code(), resolution.calls(), artifact.calls()))
        .named(&stderr)
        .is_equal_to((
            Some(1),
            registry.expected_calls().0,
            registry.expected_calls().1,
        ));
    let mut json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("stdout isn't JSON ({err})\nstderr: {stderr}"));
    redact(&mut json, &host, level.home().as_str());
    json
}

/// Replaces what differs between runs and platforms: the mock registry's
/// address, the target triple in its URLs, the temporary home, and the path
/// separator after it.
fn redact(value: &mut Value, host: &str, home: &str) {
    match value {
        Value::String(text) => {
            let redacted = text.replace(home, "[home]").replace(host, "[registry]");
            *text = redact_target(&normalize_home_separators(&redacted));
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| redact(value, host, home)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|value| redact(value, host, home)),
        _ => {}
    }
}

/// A path under `[home]` written with `/`, up to the end of that path, so
/// Windows and Unix render the same.
fn normalize_home_separators(text: &str) -> String {
    let Some(start) = text.find("[home]") else {
        return text.to_string();
    };
    let path_end = text[start..]
        .find(|c: char| c == '`' || c.is_whitespace())
        .map_or(text.len(), |end| start + end);
    format!(
        "{}{}{}",
        &text[..start],
        text[start..path_end].replace('\\', "/"),
        &text[path_end..]
    )
}

/// `[registry]/tar/<plugin>/<target>/...` with the target replaced.
fn redact_target(text: &str) -> String {
    const PREFIX: &str = "[registry]/tar/";
    let Some(start) = text.find(PREFIX) else {
        return text.to_string();
    };
    let after_prefix = start + PREFIX.len();
    let mut segments = text[after_prefix..].splitn(3, '/');
    match (segments.next(), segments.next(), segments.next()) {
        (Some(plugin), Some(_target), Some(rest)) => format!(
            "{}{PREFIX}{plugin}/[target]/{}",
            &text[..start],
            redact_target(rest)
        ),
        _ => text.to_string(),
    }
}

#[rstest]
#[case::resolution(Registry::CantResolve, "supergraph@2")]
#[case::download(Registry::RefusesTheArtifact, "supergraph@=2.9.3")]
#[case::installation(Registry::ServesAGarbledArtifact, "supergraph@=2.9.3")]
#[case::withdrawn(Registry::WithdrewTheRelease, "supergraph@=2.9.3")]
#[case::never_published(Registry::NeverPublishedTheRelease, "supergraph@=2.9.9")]
fn each_install_failure_reports_its_own_code(
    global_level: GlobalLevel,
    #[case] registry: Registry,
    #[case] request: &str,
) {
    let json = install(&global_level, registry, request);

    // One snapshot per case, named for the case rather than for the registry
    // behaviour, so two cases can share a behaviour.
    let case = format!("{registry:?}_{}", request.replace(['@', '='], "_"));
    insta::with_settings!({ snapshot_suffix => case }, {
        assert_json_snapshot!(json);
    });
}
