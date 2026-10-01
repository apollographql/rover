mod output;

use clap::Parser;
use output::DeletePairOutput;
use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::api_key::{
        delete::{DeleteKeyInput, run},
        pair_delete::{DeletePair, DeletePairInput, service::DELETE_PAIR_ATTEMPT_TIMEOUT},
        pair_list::OAuthClientPair,
    },
};
use serde::Serialize;
use tower::{Service, ServiceExt};

use crate::{
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
    command::api_key::{
        IdOpt, OrganizationOpt,
        pair_lookup::{PairTarget, lookup_pair, resolve_target},
    },
    options::ProfileOpt,
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Delete {
    #[clap(flatten)]
    organisation_opt: OrganizationOpt,
    #[clap(flatten)]
    id_opt: IdOpt,
}

impl Delete {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;
        let organization_id = &self.organisation_opt.organization_id;
        let id = &self.id_opt.id;

        // FR18-FR20: an `<ID>` may be a pair's client ID or an API key's ID - find out which
        // before deleting anything.
        match resolve_target(lookup_pair(&client, organization_id, id).await)? {
            PairTarget::Pair(pair) => self.delete_pair(&client, pair).await,
            PairTarget::Key => self.delete_api_key(&client).await,
        }
    }

    /// The API-key delete path, unchanged from before client-credential support existed
    /// (FR30, FR83) - including its output, which carries no `key_type`.
    async fn delete_api_key(&self, client: &StudioClient) -> RoverResult<RoverOutput> {
        let resp = run(
            DeleteKeyInput {
                organization_id: self.organisation_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
            },
            client,
        )
        .await?;
        Ok(RoverOutput::DeleteKeyResponse { id: resp.key_id })
    }

    /// FR28: deletes the pair, with no prompt - the same as deleting an API key.
    async fn delete_pair(
        &self,
        client: &StudioClient,
        pair: OAuthClientPair,
    ) -> RoverResult<RoverOutput> {
        let service = client.studio_graphql_service_with_timeout(DELETE_PAIR_ATTEMPT_TIMEOUT)?;
        let mut delete_pair = DeletePair::new(service);
        let delete_pair = delete_pair.ready().await?;
        let result = delete_pair
            .call(
                DeletePairInput::builder()
                    .organization_id(self.organisation_opt.organization_id.clone())
                    .client_id(pair.client_id.clone())
                    .build(),
            )
            .await;

        delete_pair_outcome(&self.organisation_opt.organization_id, pair, result)
    }
}

/// Turns the delete mutation's result into the command's result. Kept synchronous and separate
/// from the request so every branch is directly unit-testable.
fn delete_pair_outcome(
    organization_id: &str,
    pair: OAuthClientPair,
    result: Result<(), RoverClientError>,
) -> RoverResult<RoverOutput> {
    match result {
        Ok(()) => Ok(RoverOutput::CliOutput(Box::new(DeletePairOutput { pair }))),
        // Both mean nothing was deleted - safe to report as an ordinary failure.
        Err(
            err @ (RoverClientError::PairPermissionDenied { .. }
            | RoverClientError::OrganizationIDNotFound { .. }),
        ) => Err(err.into()),
        // Everything else (a timeout, a 5xx, a malformed response) is genuinely ambiguous:
        // `pair_delete::service`'s own doc comment on `DeletePair` requires the consumer to say
        // so rather than reporting a plain failure, since the mutation may have already
        // committed server-side before the client gave up waiting.
        Err(err) => Err(
            RoverError::new(err).with_suggestion(RoverErrorSuggestion::Adhoc(format!(
                "Whether the pair was deleted is unknown - check with `rover api-key list {organization_id}`."
            ))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;
    use crate::command::api_key::pair_lookup::test_pair;

    #[test]
    fn a_successful_delete_reports_the_pair() {
        let output = delete_pair_outcome("acme", test_pair(), Ok(()))
            .expect("expected a successful delete to succeed");

        let RoverOutput::CliOutput(output) = output else {
            panic!("expected a CliOutput");
        };
        assert_that!(output.json().unwrap()).is_equal_to(serde_json::json!({
            "id": "c_8f2a",
            "key_type": "ClientCredentials",
        }));
    }

    // Nothing was deleted, so no "outcome unknown" suggestion - just the stable E053 error.
    #[test]
    fn a_permission_denial_is_an_ordinary_failure() {
        let err = delete_pair_outcome(
            "acme",
            test_pair(),
            Err(RoverClientError::PairPermissionDenied {
                organization_id: "acme".to_string(),
            }),
        )
        .expect_err("expected a permission denial to fail");

        assert_that!(err.code().map(|code| code.to_string()))
            .is_some()
            .is_equal_to("E053".to_string());
        assert_that!(
            err.suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(Vec::<String>::new());
    }

    // A timeout or 5xx may have happened after the delete committed - say the outcome is unknown.
    #[test]
    fn an_ambiguous_failure_says_the_outcome_is_unknown() {
        let err = delete_pair_outcome(
            "acme",
            test_pair(),
            Err(RoverClientError::ClientError {
                msg: "timed out".to_string(),
            }),
        )
        .expect_err("expected an ambiguous failure to fail");

        assert_that!(
            err.suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![
            "Whether the pair was deleted is unknown - check with `rover api-key list acme`."
                .to_string(),
        ]);
    }

    // FR18: the same positional `<ORGANIZATION_ID> <ID>` as before - no flag picks the type.
    #[test]
    fn positional_organization_and_id_parse() {
        let delete = Delete::try_parse_from(["api-key delete", "acme", "c_8f2a"])
            .expect("expected <ORGANIZATION_ID> <ID> to parse");

        assert_that!(delete.organisation_opt.organization_id).is_equal_to("acme".to_string());
        assert_that!(delete.id_opt.id).is_equal_to("c_8f2a".to_string());
    }
}
