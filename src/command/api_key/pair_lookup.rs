//! Deciding whether an `<ID>` given to `rover api-key delete`/`rename` names a client-credential
//! pair or an API key (spec FR18-FR20, `specs/rover-431-identity-grant-management`).

use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::api_key::{
        pair_get::{GET_PAIR_ATTEMPT_TIMEOUT, GetOAuthClient, GetOAuthClientInput},
        pair_list::OAuthClientPair,
    },
};
use tower::{Service, ServiceExt};

/// What an `<ID>` turned out to address.
#[derive(Debug, PartialEq)]
pub(crate) enum PairTarget {
    /// A pair the caller can see (FR19).
    Pair(OAuthClientPair),
    /// Not a pair, or Rover can't tell - act on it as an API key, exactly as before
    /// client-credential support existed (FR19, FR20, FR83).
    Key,
}

/// Looks `id` up as a pair. A read, so it's composed under the retrying, per-attempt-timeout
/// Studio service rather than the no-retry one the pair mutations use.
pub(crate) async fn lookup_pair(
    client: &StudioClient,
    organization_id: &str,
    id: &str,
) -> Result<Option<OAuthClientPair>, RoverClientError> {
    let service = client.studio_graphql_service_with_attempt_timeout(GET_PAIR_ATTEMPT_TIMEOUT)?;
    lookup_pair_with_service(GetOAuthClient::new(service), organization_id, id).await
}

/// The part of [`lookup_pair`] that's generic over the lookup service, so a test can inject a
/// mock and assert on the exact request sent.
async fn lookup_pair_with_service<S>(
    mut service: S,
    organization_id: &str,
    id: &str,
) -> Result<Option<OAuthClientPair>, RoverClientError>
where
    S: Service<GetOAuthClientInput, Response = Option<OAuthClientPair>, Error = RoverClientError>,
{
    service
        .ready()
        .await?
        .call(
            GetOAuthClientInput::builder()
                .organization_id(organization_id)
                .client_id(id)
                .build(),
        )
        .await
}

/// FR19/FR20: turns the lookup's outcome into what the command should act on. Kept synchronous
/// and separate from the request itself so every branch is directly unit-testable.
///
/// - A pair → act on the pair.
/// - `None` → act on the ID as an API key. The Platform API reports "no such pair", "no
///   permission to see pairs", and "not enrolled" identically as `null`, so this covers both
///   FR19's "not a pair" and FR20's "can't tell".
/// - An unknown organization → also act as an API key, so the caller gets exactly the error the
///   key path has always reported for that (FR83), not a new one from the lookup.
/// - Anything else (a timeout, a 5xx) → fail. FR20 is explicit that a transient failure of the
///   lookup isn't "can't tell" and mustn't be guessed through.
pub(crate) fn resolve_target(
    lookup: Result<Option<OAuthClientPair>, RoverClientError>,
) -> Result<PairTarget, RoverClientError> {
    match lookup {
        Ok(Some(pair)) => Ok(PairTarget::Pair(pair)),
        Ok(None) | Err(RoverClientError::OrganizationIDNotFound { .. }) => Ok(PairTarget::Key),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use futures::future;
    use rover_client::operations::api_key::{
        pair_get::{
            get_pair_query::Variables,
            service::mock::{GetPairResp, MockGetPairInnerService},
        },
        pair_list::PairActor,
    };
    use rover_graphql::GraphQLRequest;
    use rover_tower::test::{MockCloneService, expect_poll_ready};
    use speculoos::prelude::*;

    use super::*;

    fn pair() -> OAuthClientPair {
        OAuthClientPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            created_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
            created_by: PairActor {
                id: "user-123".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![],
            scopes: vec!["rover:cli".to_string()],
        }
    }

    #[test]
    fn a_found_pair_is_acted_on_as_a_pair() {
        assert_that!(resolve_target(Ok(Some(pair()))))
            .is_ok()
            .is_equal_to(PairTarget::Pair(pair()));
    }

    // FR19/FR20: "not a pair" and "can't tell" both arrive as `None`.
    #[test]
    fn no_pair_is_acted_on_as_a_key() {
        assert_that!(resolve_target(Ok(None)))
            .is_ok()
            .is_equal_to(PairTarget::Key);
    }

    // FR83: an unknown organization falls through, so the key path reports it exactly as before.
    #[test]
    fn an_unknown_organization_is_acted_on_as_a_key() {
        let lookup = Err(RoverClientError::OrganizationIDNotFound {
            organization_id: "acme".to_string(),
        });

        assert_that!(resolve_target(lookup))
            .is_ok()
            .is_equal_to(PairTarget::Key);
    }

    // FR20: a transient lookup failure is not "can't tell" - fail rather than guess.
    #[test]
    fn a_transient_lookup_failure_fails_rather_than_guessing() {
        let lookup = Err(RoverClientError::ClientError {
            msg: "timed out".to_string(),
        });

        let err = resolve_target(lookup).expect_err("expected the lookup failure to propagate");
        assert_that!(err).matches(
            |err| matches!(err, RoverClientError::ClientError { msg } if msg == "timed out"),
        );
    }

    #[tokio::test]
    async fn lookup_sends_the_organization_and_id_and_maps_the_response() {
        let mut mock = MockGetPairInnerService::new();
        expect_poll_ready!(mock);
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
        };
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(|_| {
                let data: GetPairResp = serde_json::from_value(serde_json::json!({
                    "organization": {
                        "oauthClient": {
                            "clientId": "c_8f2a",
                            "clientName": "ci-deploy",
                            "createdAt": "2026-09-25T16:00:00Z",
                            "createdBy": { "actorId": "user-123", "type": "USER" },
                            "resources": [],
                            "scopes": ["rover:cli"]
                        }
                    }
                }))
                .unwrap();
                future::ready(Ok(data))
            });

        let response = lookup_pair_with_service(
            GetOAuthClient::new(MockCloneService::new(mock)),
            "acme",
            "c_8f2a",
        )
        .await
        .unwrap();

        assert_that!(response).is_some().is_equal_to(pair());
    }
}
