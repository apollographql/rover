//! FR69-FR71: a client-credential pair's secret may appear only in the stdout of the create or
//! rotate call that minted it - never in logging at any level, never on stderr, and never at
//! all when that call fails. Every case runs at `--log trace`, the most verbose level.

use std::process::Output;

use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::{mock_operation, run_api_key};

/// Distinctive enough that a match anywhere can only be the secret itself.
const SECRET: &str = "s_LEAK-CANARY-7f3a9c";

const ORG: &str = "acme";

fn pair_fields(secret_expires_at: Value) -> Value {
    json!({
        "clientId": "c_8f2a",
        "clientName": "ci-deploy",
        "clientSecret": SECRET,
        "secretExpiresAt": secret_expires_at,
    })
}

fn create_response(secret_expires_at: Value) -> Value {
    let mut pair = pair_fields(secret_expires_at);
    pair["resources"] = json!([{ "resourceId": "inventory", "resourceType": "GRAPH" }]);
    pair["scopes"] = json!(["rover:cli"]);
    json!({ "data": { "organization": { "createOAuthClient": pair } } })
}

fn rotate_response(secret_expires_at: Value) -> Value {
    json!({
        "data": { "organization": { "rotateOAuthClientSecret": pair_fields(secret_expires_at) } }
    })
}

/// Which minting command to run, and how its mutation is answered.
#[derive(Clone, Copy, Debug)]
enum Minting {
    Create,
    Rotate,
}

impl Minting {
    const fn operation(self) -> &'static str {
        match self {
            Self::Create => "CreatePairMutation",
            Self::Rotate => "RotatePairMutation",
        }
    }

    fn response(self, secret_expires_at: Value) -> Value {
        match self {
            Self::Create => create_response(secret_expires_at),
            Self::Rotate => rotate_response(secret_expires_at),
        }
    }

    fn args(self) -> Vec<&'static str> {
        match self {
            Self::Create => vec![
                "create",
                ORG,
                "client-credentials",
                "ci-deploy",
                "--graph-id",
                "inventory",
            ],
            Self::Rotate => vec!["rotate", ORG, "c_8f2a"],
        }
    }

    /// Runs the command at trace level against a mutation answered with `secret_expires_at`.
    fn run(self, secret_expires_at: Value, json: bool) -> Output {
        let server = httpmock::MockServer::start();
        let mutation = mock_operation(&server, self.operation(), self.response(secret_expires_at));
        let mut args = vec!["--log", "trace"];
        args.extend(self.args());
        if json {
            args.extend(["--format", "json"]);
        }

        let output = run_api_key(&server, &args);

        mutation.assert_calls(1);
        output
    }
}

fn occurrences(stream: &[u8]) -> usize {
    String::from_utf8_lossy(stream).matches(SECRET).count()
}

// FR69: on success, the secret is printed once, to stdout, and nowhere else - not in the trace
// logging of the request or the response, which goes to stderr.
#[rstest]
#[case::create_text(Minting::Create, false)]
#[case::create_json(Minting::Create, true)]
#[case::rotate_text(Minting::Rotate, false)]
#[case::rotate_json(Minting::Rotate, true)]
fn a_minted_secret_appears_once_on_stdout_and_never_on_stderr(
    #[case] minting: Minting,
    #[case] json: bool,
) {
    let output = minting.run(json!("2028-09-25T16:00:00Z"), json);

    assert_that!(output.status.success())
        .named(&format!(
            "exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_true();
    assert_that!((occurrences(&output.stdout), occurrences(&output.stderr)))
        .named("(stdout, stderr) occurrences of the secret")
        .is_equal_to((1, 0));
    if json {
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_that!(envelope["data"]["client_secret"]).is_equal_to(json!(SECRET));
    }
}

// FR71: a create or rotate that fails after the Platform API returned a secret - here, the
// response is missing the secret's expiry - prints the secret nowhere, in any format.
#[rstest]
#[case::create_text(Minting::Create, false)]
#[case::create_json(Minting::Create, true)]
#[case::rotate_text(Minting::Rotate, false)]
#[case::rotate_json(Minting::Rotate, true)]
fn a_failed_mint_never_prints_the_secret(#[case] minting: Minting, #[case] json: bool) {
    let output = minting.run(Value::Null, json);

    assert_that!(output.status.success()).is_false();
    assert_that!((occurrences(&output.stdout), occurrences(&output.stderr)))
        .named("(stdout, stderr) occurrences of the secret")
        .is_equal_to((0, 0));
}
