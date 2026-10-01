pub(crate) mod output;

use std::num::NonZeroUsize;

use clap::Parser;
use output::ListOutput;
use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::api_key::{
        list::{ApiKey, ApiKeyBackendType, ListKeysInput, run},
        pair_list::{
            LIST_PAIRS_ATTEMPT_TIMEOUT, ListOAuthClients, ListOAuthClientsError,
            ListOAuthClientsInput, ListOAuthClientsResponse,
        },
    },
};
use serde::Serialize;
use tower::{Service, ServiceExt};

use crate::{
    RoverOutput, RoverResult,
    command::api_key::{ApiKeyType, OrganizationOpt},
    options::ProfileOpt,
    utils::client::StudioClientConfig,
};

/// FR90/FR91's default `--limit`: how many client-credential pairs `rover api-key list`
/// auto-pages through before stopping and reporting a resume cursor, when `--limit` isn't
/// given. Matches `rover graph-artifact list-tags`'s own `--limit` default, for consistency
/// across the CLI's total-cap flags.
const DEFAULT_PAIRS_LIMIT: usize = 100;

#[derive(Debug, Serialize, Parser)]
pub(crate) struct List {
    #[clap(flatten)]
    organization_opt: OrganizationOpt,

    // FR11: repeatable; omitted means every type is in scope.
    #[clap(
        long = "type",
        value_name = "TYPE",
        value_enum,
        help = "Only report keys/pairs of this type (repeatable; default: every type)"
    )]
    type_filter: Vec<ApiKeyType>,

    /// Resume client-credential pairs enumeration from this cursor (spec FR90/FR91) - a previous
    /// invocation's `client_credentials_next_after`, or the text output's resume note. Has no
    /// effect when `--type` excludes `client-credentials`.
    #[clap(long)]
    after: Option<String>,

    /// Collect at most this many client-credential pairs before returning (spec FR90/FR91). An
    /// organization with more pairs than this still succeeds: it reports what it collected plus
    /// a cursor to resume from with `--after`. Has no effect when `--type` excludes
    /// `client-credentials`. Rejected at parse time if `0` - a cap of zero could never report
    /// FR90's "every pair collected" correctly, since it would never make a single request.
    #[clap(
        long,
        default_value_t = NonZeroUsize::new(DEFAULT_PAIRS_LIMIT).expect("DEFAULT_PAIRS_LIMIT is nonzero")
    )]
    limit: NonZeroUsize,
}

impl List {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;
        let organization_id = self.organization_opt.organization_id.clone();

        let keys_in_scope =
            self.in_scope(ApiKeyType::Operator) || self.in_scope(ApiKeyType::Subgraph);
        let pairs_in_scope = self.in_scope(ApiKeyType::ClientCredentials);

        // FR10: keys are always fetched, regardless of `--type` - a failure here fails the whole
        // command with no output, exactly as it does today. `--type` only decides which of the
        // already-fetched keys are reported (`reported_keys`) and whether `keys` appears in the
        // output at all (`keys_in_scope`).
        let all_keys = run(
            ListKeysInput {
                organization_id: organization_id.clone(),
            },
            &client,
        )
        .await?
        .keys;
        let reported_keys: Vec<ApiKey> = all_keys
            .into_iter()
            .filter(|key| self.key_in_scope(key))
            .collect();

        let pairs = if pairs_in_scope {
            Some(
                fetch_pairs(
                    &client,
                    organization_id.clone(),
                    self.after.clone(),
                    self.limit.get(),
                )
                .await,
            )
        } else {
            None
        };

        build_output(keys_in_scope, reported_keys, organization_id, pairs)
    }

    fn in_scope(&self, key_type: ApiKeyType) -> bool {
        self.type_filter.is_empty() || self.type_filter.contains(&key_type)
    }

    fn key_in_scope(&self, key: &ApiKey) -> bool {
        if self.type_filter.is_empty() {
            return true;
        }
        self.type_filter.iter().any(|t| {
            matches!(
                (t, &key.key_type),
                (ApiKeyType::Operator, Some(ApiKeyBackendType::Operator))
                    | (ApiKeyType::Subgraph, Some(ApiKeyBackendType::Subgraph))
            )
        })
    }
}

/// Collects up to `limit` of an organization's client-credential pairs, resuming from `after`
/// when given, via [`ListOAuthClients`] (spec FR90, FR91) - it already pages internally until it
/// has `limit` pairs in total, or the organization is exhausted, so no further pagination layer
/// is needed here. Composes a per-attempt [`TimeoutLayer`](rover_http::timeout::TimeoutLayer)
/// inside the retry budget `StudioClient::studio_graphql_service_with_attempt_timeout` already
/// applies, per this repo's network-call composition guidance (AGENTS.md) - this is a read, safe
/// to retry.
async fn fetch_pairs(
    client: &StudioClient,
    organization_id: String,
    after: Option<String>,
    limit: usize,
) -> Result<ListOAuthClientsResponse, ListOAuthClientsError> {
    let service = client
        .studio_graphql_service_with_attempt_timeout(LIST_PAIRS_ATTEMPT_TIMEOUT)
        .map_err(|err| ListOAuthClientsError::Other(RoverClientError::from(err)))?;
    fetch_pairs_with_service(
        ListOAuthClients::new(service),
        organization_id,
        after,
        limit,
    )
    .await
}

/// The part of [`fetch_pairs`] that's generic over the pairs service, rather than a concrete
/// `StudioClient`-backed one - so a test can inject a mock in its place and assert on the exact
/// `ListOAuthClientsInput` the command builds from `--after`/`--limit`, and that pagination
/// actually respects `--limit`'s value, not just that clap parses the flag.
async fn fetch_pairs_with_service<S>(
    mut service: S,
    organization_id: String,
    after: Option<String>,
    limit: usize,
) -> Result<ListOAuthClientsResponse, ListOAuthClientsError>
where
    S: Service<
            ListOAuthClientsInput,
            Response = ListOAuthClientsResponse,
            Error = ListOAuthClientsError,
        >,
{
    service
        .ready()
        .await?
        .call(
            ListOAuthClientsInput::builder()
                .organization_id(organization_id)
                .maybe_after(after)
                .limit(limit)
                .build(),
        )
        .await
}

/// The decision logic behind `List::run` - kept separate and synchronous (no `Service`, no
/// `.await`) so every branch is directly unit-testable against an already-resolved `pairs`
/// outcome, with no network mocking required. `pairs: None` means `--type` excluded
/// `client-credentials` entirely (FR11); `Some(Ok(_))` is a clean fetch (possibly capped, FR90);
/// `Some(Err(_))` is FR16 (keys still in scope - reported best-effort alongside the failure) or
/// FR17 (keys excluded too - `keys_in_scope` is `false`, so nothing is reported at all), which
/// this function doesn't need to distinguish beyond that flag - both become
/// `RoverClientError::PairListFailure`, differing only in whether `keys` is `Some` or `None`.
fn build_output(
    keys_in_scope: bool,
    reported_keys: Vec<ApiKey>,
    organization_id: String,
    pairs: Option<Result<ListOAuthClientsResponse, ListOAuthClientsError>>,
) -> RoverResult<RoverOutput> {
    match pairs {
        None => Ok(RoverOutput::CliOutput(Box::new(ListOutput {
            keys: keys_in_scope.then_some(reported_keys),
            pairs: None,
            pairs_next_after: None,
        }))),
        Some(Ok(response)) => Ok(RoverOutput::CliOutput(Box::new(ListOutput {
            keys: keys_in_scope.then_some(reported_keys),
            pairs: Some(response.pairs),
            pairs_next_after: response.next_after,
        }))),
        // FR16/FR17: `RoverError::print()`/`get_internal_data_json()` (src/error/mod.rs)
        // special-case `PairListFailure` to still surface `keys` when it's `Some`, and to report
        // nothing at all when it's `None` - so this one arm covers both specs.
        Some(Err(err)) => Err(RoverClientError::PairListFailure {
            organization_id,
            keys: keys_in_scope.then_some(reported_keys),
            source: Box::new(err),
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_client::operations::api_key::{
        list::ApiKey,
        pair_list::service::mock::{ListPairsResp, MockListPairsInnerService},
    };
    use rover_tower::test::{MockCloneService, expect_poll_ready};
    use speculoos::prelude::*;

    use super::*;

    fn list(type_filter: Vec<ApiKeyType>) -> List {
        List {
            organization_opt: OrganizationOpt {
                organization_id: "acme".to_string(),
            },
            type_filter,
            after: None,
            limit: NonZeroUsize::new(DEFAULT_PAIRS_LIMIT).unwrap(),
        }
    }

    fn key_of_type(key_type: ApiKeyBackendType) -> ApiKey {
        ApiKey {
            created_at: chrono::DateTime::parse_from_rfc3339("2026-01-04T12:00:00Z").unwrap(),
            expires_at: None,
            id: "key-123".to_string(),
            name: None,
            key_type: Some(key_type),
        }
    }

    #[test]
    fn every_type_is_in_scope_when_the_filter_is_empty() {
        let list = list(vec![]);

        assert_that!(list.in_scope(ApiKeyType::Operator)).is_true();
        assert_that!(list.in_scope(ApiKeyType::Subgraph)).is_true();
        assert_that!(list.in_scope(ApiKeyType::ClientCredentials)).is_true();
    }

    #[test]
    fn only_the_named_types_are_in_scope_when_the_filter_is_set() {
        let list = list(vec![ApiKeyType::Operator]);

        assert_that!(list.in_scope(ApiKeyType::Operator)).is_true();
        assert_that!(list.in_scope(ApiKeyType::Subgraph)).is_false();
        assert_that!(list.in_scope(ApiKeyType::ClientCredentials)).is_false();
    }

    #[test]
    fn key_in_scope_matches_by_key_type() {
        let list = list(vec![ApiKeyType::Operator]);

        assert_that!(list.key_in_scope(&key_of_type(ApiKeyBackendType::Operator))).is_true();
        assert_that!(list.key_in_scope(&key_of_type(ApiKeyBackendType::Subgraph))).is_false();
    }

    #[test]
    fn key_in_scope_is_permissive_with_an_empty_filter() {
        let list = list(vec![]);

        assert_that!(list.key_in_scope(&key_of_type(ApiKeyBackendType::Gateway))).is_true();
    }

    #[test]
    fn client_credentials_is_a_recognized_clap_type_value() {
        let list = List::try_parse_from(["api-key list", "acme", "--type", "client-credentials"])
            .expect("expected `client-credentials` to parse as a valid --type value");

        assert_that!(list.type_filter).is_equal_to(vec![ApiKeyType::ClientCredentials]);
    }

    #[test]
    fn type_is_repeatable() {
        let list = List::try_parse_from([
            "api-key list",
            "acme",
            "--type",
            "operator",
            "--type",
            "client-credentials",
        ])
        .expect("expected repeated --type flags to parse");

        assert_that!(list.type_filter)
            .is_equal_to(vec![ApiKeyType::Operator, ApiKeyType::ClientCredentials]);
    }

    // FR91: `--limit` defaults to FR90's 100 when not given.
    #[test]
    fn limit_defaults_to_the_spec_value_when_omitted() {
        let list = List::try_parse_from(["api-key list", "acme"])
            .expect("expected `rover api-key list <ORG>` to parse with no --limit given");

        assert_that!(list.limit.get()).is_equal_to(DEFAULT_PAIRS_LIMIT);
        assert_that!(list.after).is_none();
    }

    // FR91: `--after`/`--limit` are accepted and parsed through to their fields.
    #[test]
    fn after_and_limit_are_recognized_clap_flags() {
        let list = List::try_parse_from([
            "api-key list",
            "acme",
            "--after",
            "cursor-1",
            "--limit",
            "25",
        ])
        .expect("expected --after/--limit to parse");

        assert_that!(list.after)
            .is_some()
            .is_equal_to("cursor-1".to_string());
        assert_that!(list.limit.get()).is_equal_to(25);
    }

    // FR90: a cap of zero could never correctly report "every pair collected" (it would never
    // make a single request), so it's rejected at parse time rather than silently misreported.
    #[test]
    fn limit_zero_is_rejected_at_parse_time() {
        let result = List::try_parse_from(["api-key list", "acme", "--limit", "0"]);

        let error = result.expect_err("expected --limit 0 to be rejected");
        assert_that!(error.kind()).is_equal_to(clap::error::ErrorKind::ValueValidation);
    }

    fn raw_pair_page(has_next: bool, end_cursor: Option<&str>, client_id: &str) -> ListPairsResp {
        serde_json::from_value(serde_json::json!({
            "organization": {
                "oauthClients": {
                    "pageInfo": {
                        "endCursor": end_cursor,
                        "hasNextPage": has_next
                    },
                    "edges": [{
                        "node": {
                            "clientId": client_id,
                            "clientName": "ci-deploy",
                            "createdAt": "2026-09-25T16:00:00Z",
                            "createdBy": { "actorId": "user-123", "type": "USER" },
                            "resources": [],
                            "scopes": ["rover:cli"]
                        }
                    }]
                }
            }
        }))
        .unwrap()
    }

    // FR90/FR91: this is the one place `--limit`'s value, the real `ListOAuthClients` service,
    // and the command's own request-building (`fetch_pairs_with_service`) are all exercised
    // together - the clap-parsing tests above only prove the flag parses, and
    // `pair_list::service`'s own tests only prove `ListOAuthClients` respects `limit` in
    // isolation. Three raw pages are available (`c_1`/`c_2`/`c_3`), each with its own
    // `hasNextPage: true`, but `--limit 1`'s worth of request should still stop after the
    // first - proving the limit reaches the service and is actually respected, not just parsed.
    #[tokio::test]
    async fn fetch_pairs_sends_the_requested_limit_and_the_service_respects_it() {
        let pages = std::sync::Arc::new(std::sync::Mutex::new(vec![
            raw_pair_page(true, Some("cursor-1"), "c_1"),
            raw_pair_page(true, Some("cursor-2"), "c_2"),
            raw_pair_page(false, None, "c_3"),
        ]));

        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).returning(move |_| {
            let mut pages = pages.lock().unwrap();
            future::ready(Ok(pages.remove(0)))
        });

        let service = ListOAuthClients::new(MockCloneService::new(mock));
        let response = fetch_pairs_with_service(service, "acme".to_string(), None, 1)
            .await
            .unwrap();

        // Full-value, not just the client ID: a regression that dropped or mis-mapped a field
        // while still returning the right *count* of pairs would slip past a shallower check.
        assert_that!(response.pairs).is_equal_to(vec![
            rover_client::operations::api_key::pair_list::OAuthClientPair {
                client_id: "c_1".to_string(),
                name: Some("ci-deploy".to_string()),
                created_at: chrono::DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
                created_by: rover_client::operations::api_key::pair_list::PairActor {
                    id: "user-123".to_string(),
                    kind: "user".to_string(),
                },
                resources: vec![],
                scopes: vec!["rover:cli".to_string()],
            },
        ]);
        assert_that!(response.next_after)
            .is_some()
            .is_equal_to("cursor-1".to_string());
    }

    fn pair(client_id: &str) -> rover_client::operations::api_key::pair_list::OAuthClientPair {
        rover_client::operations::api_key::pair_list::OAuthClientPair {
            client_id: client_id.to_string(),
            name: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-01-04T12:00:00Z").unwrap(),
            created_by: rover_client::operations::api_key::pair_list::PairActor {
                id: "user-123".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![],
            scopes: vec![],
        }
    }

    fn a_key() -> ApiKey {
        key_of_type(ApiKeyBackendType::Operator)
    }

    /// The full JSON rendering of `a_key()`, for whole-value assertions.
    fn a_key_json() -> serde_json::Value {
        serde_json::json!({
            "created_at": "2026-01-04T12:00:00Z",
            "expires_at": null,
            "id": "key-123",
            "name": null,
            "key_type": "Operator",
        })
    }

    // FR11: pairs out of scope (`--type` excluded `client-credentials`) reports no pairs at all,
    // regardless of whether keys are in scope.
    #[test]
    fn pairs_out_of_scope_reports_no_pairs() {
        let result = build_output(true, vec![a_key()], "acme".to_string(), None);

        let output = result.expect("expected Ok when pairs aren't in scope");
        let RoverOutput::CliOutput(output) = output else {
            panic!("expected a CliOutput");
        };

        assert_that!(output.json().unwrap()).is_equal_to(serde_json::json!({
            "keys": [a_key_json()],
        }));
    }

    // A clean pairs fetch reports both keys and pairs, with no resume cursor once everything's
    // been collected.
    #[test]
    fn a_clean_fetch_reports_keys_and_pairs() {
        let response = ListOAuthClientsResponse {
            pairs: vec![pair("c_1")],
            next_after: None,
        };
        let result = build_output(true, vec![a_key()], "acme".to_string(), Some(Ok(response)));

        let output = result.expect("expected Ok for a clean fetch");
        let RoverOutput::CliOutput(output) = output else {
            panic!("expected a CliOutput");
        };

        assert_that!(output.json().unwrap()).is_equal_to(serde_json::json!({
            "keys": [a_key_json()],
            "client_credentials": [
                {
                    "key_type": "ClientCredentials",
                    "id": "c_1",
                    "client_id": "c_1",
                    "name": null,
                    "graphs": [],
                    "scopes": [],
                    "created_at": "2026-01-04T12:00:00Z",
                    "created_by": { "id": "user-123", "type": "user" },
                }
            ],
            "client_credentials_next_after": null,
        }));
    }

    // FR16: keys are still in scope - a pairs failure is reported best-effort alongside a loud
    // failure that carries E056, not a bare, uncoded error (the bug this test guards against).
    #[test]
    fn fr16_a_pairs_failure_with_keys_in_scope_keeps_the_keys_and_gets_e056() {
        let result = build_output(
            true,
            vec![a_key()],
            "acme".to_string(),
            Some(Err(ListOAuthClientsError::MissingCursor)),
        );

        let error = result.expect_err("expected an Err for a pairs failure");
        assert_that!(error.code().map(|code| code.to_string()))
            .is_some()
            .is_equal_to("E056".to_string());
        assert_that!(error.get_internal_data_json()).is_equal_to(serde_json::json!({
            "keys": [a_key_json()],
            "client_credentials": null,
            "client_credentials_next_after": null,
        }));
    }

    // FR17: keys are excluded too (`--type client-credentials` only) - the same failure reports
    // no keys at all, but still carries E056, not a bare, uncoded error.
    #[test]
    fn fr17_a_pairs_failure_with_keys_out_of_scope_reports_nothing_but_still_gets_e056() {
        let result = build_output(
            false,
            vec![a_key()],
            "acme".to_string(),
            Some(Err(ListOAuthClientsError::MissingCursor)),
        );

        let error = result.expect_err("expected an Err for a pairs failure");
        assert_that!(error.code().map(|code| code.to_string()))
            .is_some()
            .is_equal_to("E056".to_string());
        assert_that!(error.get_internal_data_json()).is_equal_to(serde_json::json!({
            "client_credentials": null,
            "client_credentials_next_after": null,
        }));
    }
}
