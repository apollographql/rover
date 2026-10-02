use clap::Parser;
use rover_client::{
    RoverClientError,
    operations::api_key::{
        get::{GetKeyInput, run as run_get},
        pair_list::OAuthClientPair,
        rename::{RenameKeyInput, run as run_rename},
    },
};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::api_key::{IdOpt, OrganizationOpt, pair_lookup::PairTarget},
    options::ProfileOpt,
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Rename {
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
    #[clap(flatten)]
    id_opt: IdOpt,
    #[clap(help = "The new name of the key once it has been renamed")]
    new_name: String,
}

impl Rename {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;

        // FR18-FR20, FR31: a pair can't be renamed, so find out whether `<ID>` is one before
        // changing anything. Not a pair, or Rover can't tell, renames it as an API key exactly
        // as before.
        let target = PairTarget::lookup(
            &client,
            &self.organization_opt.organization_id,
            &self.id_opt.id,
        )
        .await?;
        target.refuse_pairs()?;

        let old_key_resp = run_get(
            GetKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
            },
            &client,
        )
        .await?;

        let rename_resp = run_rename(
            RenameKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
                new_name: self.new_name.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::RenameKeyResponse {
            id: rename_resp.key_id,
            old_name: old_key_resp.key.name,
            new_name: rename_resp.name,
        })
    }
}

impl PairTarget {
    /// FR31: fails, without making any change, when the ID turned out to be a pair. Kept
    /// separate from the request so both branches are directly unit-testable.
    fn refuse_pairs(self) -> Result<(), RoverClientError> {
        match self {
            PairTarget::Pair(OAuthClientPair { client_id, .. }) => {
                Err(RoverClientError::PairCannotBeRenamed { client_id })
            }
            PairTarget::Key => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;
    use crate::{
        RoverError, RoverErrorCode, command::api_key::pair_lookup::tests::test_pair,
        options::JsonOutput,
    };

    fn refused() -> RoverError {
        RoverError::new(
            PairTarget::Pair(test_pair())
                .refuse_pairs()
                .expect_err("expected a pair to be refused"),
        )
    }

    // FR31: a pair is refused with the exact required text and its own stable code.
    #[test]
    fn a_pair_is_refused_with_e059() {
        let err = refused();

        assert_that!(err.message()).is_equal_to(
            "`c_8f2a` is a client-credential pair. Client-credential pairs can't be renamed."
                .to_string(),
        );
        assert_that!(err.code()).is_equal_to(Some(RoverErrorCode::E059));
    }

    // FR77: the refusal goes through the standard `--format json` envelope, with `error.code`
    // pinned end to end.
    #[test]
    fn a_pair_refusal_json_envelope_snapshot() {
        insta::assert_json_snapshot!(JsonOutput::from(&refused()));
    }

    // FR19/FR20/FR83: not a pair, or can't tell - rename it as an API key, as before.
    #[test]
    fn a_key_is_renamed_as_before() {
        assert_that!(PairTarget::Key.refuse_pairs())
            .is_ok()
            .is_equal_to(());
    }
}
