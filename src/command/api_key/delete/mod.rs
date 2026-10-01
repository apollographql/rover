mod output;

use clap::Parser;
use output::DeletePairOutput;
use rover_client::{
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
    RoverOutput, RoverResult,
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

        let output = DeletePairOutput::new(&self.organisation_opt.organization_id, pair, result)?;
        Ok(RoverOutput::CliOutput(Box::new(output)))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    // FR18: the same positional `<ORGANIZATION_ID> <ID>` as before - no flag picks the type.
    #[test]
    fn positional_organization_and_id_parse() {
        let delete = Delete::try_parse_from(["api-key delete", "acme", "c_8f2a"])
            .expect("expected <ORGANIZATION_ID> <ID> to parse");

        assert_that!(delete.organisation_opt.organization_id).is_equal_to("acme".to_string());
        assert_that!(delete.id_opt.id).is_equal_to("c_8f2a".to_string());
    }
}
