//! A client-credential pair's full lifecycle against live Studio - create, list, rotate,
//! delete - pinning each step's JSON payload (spec §8 of
//! `specs/rover-431-identity-grant-management`: FR8, FR15, FR26, FR30).
//!
//! Managing pairs needs an organization admin's key on an organization enrolled in
//! client-credential support, which the shared e2e key isn't. The test reads its own:
//! - `APOLLO_KEY_ROVER_E2E_ORG_ADMIN`: the admin's key;
//! - `ROVER_E2E_ORG_ID`: the enrolled organization;
//! - `ROVER_E2E_GRAPH_ID` (optional, default `rover-e2e-tests`): a graph in it to scope the pair to.
//!
//! Without the first two, it skips rather than fails, so the smoke suite stays green until
//! they're provisioned.
//!
//! Pair mutations are rate limited per organization, so the lifecycle runs on one platform only
//! (Linux x86_64) - pairs behave the same whichever OS Rover runs on - and a step the Platform API
//! rate limits is retried with backoff.

use std::{
    env, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_json_snapshot;
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

/// How long to wait before each retry of a rate-limited step. A rate-limited mutation never
/// reached its resolver, so retrying it can't create, rotate, or delete twice.
const BACKOFF: [Duration; 5] = [
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(40),
    Duration::from_secs(60),
];

/// How the Platform API's rate limiter phrases its GraphQL error.
const RATE_LIMITED: &str = "Rate limit exceeded";

/// Every pair this test creates is named this, then the creation time in Unix nanoseconds.
const NAME_PREFIX: &str = "rover-e2e-";

/// A test pair older than this was left behind by an earlier run, not made by one still going.
const STALE_AFTER: Duration = Duration::from_secs(60 * 60);

struct Org {
    admin_key: String,
    organization_id: String,
    graph_id: String,
}

impl Org {
    fn from_env() -> Option<Self> {
        Some(Self {
            admin_key: non_empty_var("APOLLO_KEY_ROVER_E2E_ORG_ADMIN")?,
            organization_id: non_empty_var("ROVER_E2E_ORG_ID")?,
            graph_id: non_empty_var("ROVER_E2E_GRAPH_ID")
                .unwrap_or_else(|| "rover-e2e-tests".to_string()),
        })
    }

    /// Runs `rover api-key <args> --format json` as the admin and returns its JSON envelope,
    /// retrying with [`BACKOFF`] while the Platform API rate limits it. Any other failure, or one
    /// that's still rate limited after every retry, is returned as a description.
    fn try_api_key(&self, args: &[&str]) -> Result<Value, String> {
        let mut waits = BACKOFF.iter();
        loop {
            let output = cargo_bin_cmd!("rover")
                .env("APOLLO_KEY", &self.admin_key)
                .env_remove("APOLLO_CLIENT_ID")
                .env_remove("APOLLO_CLIENT_SECRET")
                .args(["--skip-update-check", "api-key"])
                .args(args)
                .args(["--format", "json"])
                .output()
                .unwrap();
            let envelope: Option<Value> = serde_json::from_slice(&output.stdout).ok();
            if output.status.success() {
                return envelope.ok_or_else(|| "a successful run printed no JSON".to_string());
            }
            let rate_limited = envelope
                .as_ref()
                .and_then(|envelope| envelope["error"]["message"].as_str())
                .is_some_and(|message| message.contains(RATE_LIMITED));
            match waits.next() {
                Some(wait) if rate_limited => {
                    eprintln!(
                        "`rover api-key {}` was rate limited; retrying in {}s",
                        args[0],
                        wait.as_secs()
                    );
                    thread::sleep(*wait);
                }
                // With `--format json` the error is reported on stdout, in the envelope. A failed
                // create or rotate never prints its secret (FR71), so stdout is safe to show here.
                _ => {
                    return Err(format!(
                        "`rover api-key {}` failed (stdout: {}, stderr: {})",
                        args.join(" "),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
            }
        }
    }

    /// [`Self::try_api_key`], failing the test if the step fails.
    fn api_key(&self, args: &[&str]) -> Value {
        self.try_api_key(args)
            .unwrap_or_else(|failure| panic!("{failure}"))
    }

    /// Deletes test pairs an earlier run left behind - one whose cleanup was itself rate limited,
    /// say. Only pairs older than [`STALE_AFTER`] are touched, so a run still in progress
    /// elsewhere keeps its pair. Best-effort: a failure here doesn't fail the test.
    fn delete_stale_pairs(&self, now: u128) {
        let organization_id = self.organization_id.as_str();
        let Ok(listed) =
            self.try_api_key(&["list", organization_id, "--type", "client-credentials"])
        else {
            return;
        };
        let pairs = listed["data"]["client_credentials"].as_array().cloned();
        for pair in pairs.unwrap_or_default() {
            let created = pair["name"]
                .as_str()
                .and_then(|name| name.strip_prefix(NAME_PREFIX))
                .and_then(|nanos| nanos.parse::<u128>().ok());
            let (Some(created), Some(client_id)) = (created, pair["client_id"].as_str()) else {
                continue;
            };
            if now.saturating_sub(created) > STALE_AFTER.as_nanos() {
                let _ = self.try_api_key(&["delete", organization_id, client_id]);
            }
        }
    }
}

/// GitHub Actions passes an unprovisioned secret or variable as an empty string, not as unset.
fn non_empty_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// Deletes the pair if the test ends before its own `delete` step does, so a failed run
/// doesn't leave pairs behind in the shared organization.
struct Cleanup<'a> {
    org: &'a Org,
    client_id: Option<String>,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        if let Some(client_id) = self.client_id.take() {
            let _ = self
                .org
                .try_api_key(&["delete", &self.org.organization_id, &client_id]);
        }
    }
}

/// Replaces every value that differs between runs, so the rest of the payload can be pinned.
fn redact(mut value: Value, fields: &[&str]) -> Value {
    for field in fields {
        if let Some(slot) = value.get_mut(*field)
            && !slot.is_null()
        {
            *slot = json!(format!("[{field}]"));
        }
    }
    value
}

#[rstest]
#[ignore]
fn e2e_test_rover_api_key_client_credentials_lifecycle() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        eprintln!(
            "skipping: the client-credential pair lifecycle runs on Linux x86_64 only, to stay \
            within the Platform API's rate limit"
        );
        return;
    }
    let Some(org) = Org::from_env() else {
        eprintln!(
            "skipping: set APOLLO_KEY_ROVER_E2E_ORG_ADMIN and ROVER_E2E_ORG_ID to run the \
            client-credential pair lifecycle"
        );
        return;
    };
    let org_id = org.organization_id.as_str();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("{NAME_PREFIX}{unique}");
    org.delete_stale_pairs(unique);

    // FR8: create.
    let created = org.api_key(&[
        "create",
        org_id,
        "client-credentials",
        &name,
        "--graph-id",
        &org.graph_id,
        "--secret-lifetime-days",
        "1",
    ]);
    let client_id = created["data"]["client_id"].as_str().unwrap().to_string();
    let mut cleanup = Cleanup {
        org: &org,
        client_id: Some(client_id.clone()),
    };
    assert_that!(created["data"]["client_secret"].as_str())
        .is_some()
        .matches(|secret| !secret.is_empty());
    assert_json_snapshot!(
        "create",
        redact(
            created["data"].clone(),
            &[
                "id",
                "client_id",
                "client_secret",
                "secret_expires_at",
                "name",
                "graphs"
            ],
        )
    );

    // FR15: the new pair is listed, without its secret.
    let listed = org.api_key(&["list", org_id, "--type", "client-credentials"]);
    let entry = listed["data"]["client_credentials"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pair| pair["client_id"] == json!(client_id))
        .cloned()
        .expect("the new pair is listed");
    assert_json_snapshot!(
        "list_entry",
        redact(
            entry,
            &[
                "id",
                "client_id",
                "name",
                "graphs",
                "created_at",
                "created_by"
            ],
        )
    );

    // FR26: rotate, with a grace period so the old secret's cutoff is reported.
    let rotated = org.api_key(&["rotate", org_id, &client_id, "--grace-period-days", "1"]);
    assert_that!(rotated["data"]["client_secret"])
        .is_not_equal_to(created["data"]["client_secret"].clone());
    assert_json_snapshot!(
        "rotate",
        redact(
            rotated["data"].clone(),
            &[
                "id",
                "client_id",
                "client_secret",
                "secret_expires_at",
                "previous_secrets_expire_at",
            ],
        )
    );

    // FR30: delete, then confirm it's gone.
    let deleted = org.api_key(&["delete", org_id, &client_id]);
    cleanup.client_id = None;
    assert_json_snapshot!("delete", redact(deleted["data"].clone(), &["id"]));
    let after = org.api_key(&["list", org_id, "--type", "client-credentials"]);
    assert_that!(
        after["data"]["client_credentials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pair| pair["client_id"] == json!(client_id))
    )
    .is_false();
}
