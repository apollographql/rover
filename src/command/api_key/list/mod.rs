pub(crate) mod output;

use clap::Parser;
use output::ListOutput;
use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::api_key::{
        list::{ApiKey, ListKeysInput, run},
        pair_list::{
            ListOAuthClients, ListOAuthClientsError, ListOAuthClientsInput, OAuthClientPair,
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

        if !pairs_in_scope {
            return Ok(RoverOutput::CliOutput(Box::new(ListOutput {
                keys: keys_in_scope.then_some(reported_keys),
                pairs: None,
            })));
        }

        match fetch_all_pairs(&client, organization_id.clone()).await {
            Ok(pairs) => Ok(RoverOutput::CliOutput(Box::new(ListOutput {
                keys: keys_in_scope.then_some(reported_keys),
                pairs: Some(pairs),
            }))),
            // FR17: pairs were the *only* thing in scope - nothing to show best-effort, so the
            // raw pairs error propagates with no special handling, same as any ordinary failure.
            Err(err) if !keys_in_scope => Err(err.into()),
            // FR16: keys are still in scope - report them best-effort, and classify this as
            // PairListFailure so `RoverError::print()`/`get_internal_data_json()` (src/error/mod.rs)
            // can still surface them alongside the loud failure.
            Err(err) => Err(RoverClientError::PairListFailure {
                organization_id,
                keys: reported_keys,
                source: Box::new(err),
            }
            .into()),
        }
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
                (t, key.key_type.as_deref()),
                (ApiKeyType::Operator, Some("Operator")) | (ApiKeyType::Subgraph, Some("Subgraph"))
            )
        })
    }
}

/// Pages through every page of an organization's client-credential pairs before returning -
/// `ListOAuthClients`'s own doc comment is explicit that this is the caller's job (spec FR10).
async fn fetch_all_pairs(
    client: &StudioClient,
    organization_id: String,
) -> Result<Vec<OAuthClientPair>, ListOAuthClientsError> {
    let service = client
        .studio_graphql_service()
        .map_err(|err| ListOAuthClientsError::Other(RoverClientError::from(err)))?;
    let mut list_pairs = ListOAuthClients::new(service);

    let mut pairs = Vec::new();
    let mut after = None;
    loop {
        let ready = list_pairs.ready().await?;
        let response = ready
            .call(
                ListOAuthClientsInput::builder()
                    .organization_id(organization_id.clone())
                    .maybe_after(after.clone())
                    .build(),
            )
            .await?;
        pairs.extend(response.pairs);
        match response.next_after {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use rover_client::operations::api_key::list::ApiKey;
    use speculoos::prelude::*;

    use super::*;

    fn list(type_filter: Vec<ApiKeyType>) -> List {
        List {
            organization_opt: OrganizationOpt {
                organization_id: "acme".to_string(),
            },
            type_filter,
        }
    }

    fn key_of_type(key_type: &str) -> ApiKey {
        ApiKey {
            created_at: chrono::DateTime::parse_from_rfc3339("2026-01-04T12:00:00Z").unwrap(),
            expires_at: None,
            id: "key-123".to_string(),
            name: None,
            key_type: Some(key_type.to_string()),
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
    fn key_in_scope_matches_by_rendered_key_type() {
        let list = list(vec![ApiKeyType::Operator]);

        assert_that!(list.key_in_scope(&key_of_type("Operator"))).is_true();
        assert_that!(list.key_in_scope(&key_of_type("Subgraph"))).is_false();
    }

    #[test]
    fn key_in_scope_is_permissive_with_an_empty_filter() {
        let list = list(vec![]);

        assert_that!(list.key_in_scope(&key_of_type("Gateway"))).is_true();
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
}
