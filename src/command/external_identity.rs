use anyhow::anyhow;
use rover_auth::external_token::ExternalAccessToken;
use rover_client::{
    RoverClientError,
    blocking::ExternalTokenCapture,
    operations::config::who_am_i::{
        DEFAULT_ATTEMPT_TIMEOUT, RegistryIdentity, WhoAmI, WhoAmIRequest,
    },
};
use rover_std::Style;
use tower::ServiceExt;

use crate::{
    RoverResult,
    command::CliOutput,
    options::ProfileOpt,
    utils::{client::StudioClientConfig, table},
};

/// Shared Platform API identity output, with no inferred OAuth grant type.
#[derive(Debug)]
pub(crate) struct ExternalIdentityOutput {
    identity: RegistryIdentity,
    origin: String,
    unmasked_token: Option<ExternalAccessToken>,
}

impl ExternalIdentityOutput {
    pub(crate) async fn fetch(
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
        insecure_unmask_key: bool,
    ) -> RoverResult<Self> {
        let client = client_config.get_authenticated_client(profile)?;
        let origin = client
            .external_token_source()
            .ok_or_else(|| anyhow!("An external credential source was not selected."))?
            .origin()
            .to_string();
        let capture = insecure_unmask_key.then(ExternalTokenCapture::default);
        let service = match &capture {
            Some(capture) => client.studio_graphql_service_with_external_token_capture(
                capture.clone(),
                DEFAULT_ATTEMPT_TIMEOUT,
            )?,
            None => client.studio_graphql_service_with_attempt_timeout(DEFAULT_ATTEMPT_TIMEOUT)?,
        };
        let identity = WhoAmI::new(service)
            .oneshot(WhoAmIRequest::new(client.get_credential_origin()))
            .await
            // Both a missing identity and a body-level rejection arrive as an invalid key.
            .map_err(|error| match RoverClientError::from(error) {
                RoverClientError::InvalidKey => {
                    crate::RoverError::new(rover_auth::external_auth::ExternalAuthError::Rejected)
                }
                error => crate::RoverError::from(error),
            })?;
        let unmasked_token = capture
            .map(|capture| {
                capture.take().ok_or_else(|| {
                    anyhow!("The successful identity lookup did not capture its credential.")
                })
            })
            .transpose()?;
        Ok(Self {
            identity,
            origin,
            unmasked_token,
        })
    }

    fn access_token(&self) -> &str {
        self.unmasked_token
            .as_ref()
            .map_or("[REDACTED]", ExternalAccessToken::secret)
    }
}

impl CliOutput for ExternalIdentityOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();
        table.add_row(vec![
            Style::WhoAmIKey.paint("Actor Type"),
            self.identity.key_actor_type.to_string(),
        ]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Identity ID"),
            &self.identity.id,
        ]);
        if let Some(title) = &self.identity.graph_title {
            table.add_row(vec![&Style::WhoAmIKey.paint("Graph Title"), title]);
        }
        table.add_row(vec![&Style::WhoAmIKey.paint("Origin"), &self.origin]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Access Token"),
            self.access_token(),
        ]);
        table.to_string()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "actor_type": self.identity.key_actor_type.to_string(),
            "id": self.identity.id,
            "graph_title": self.identity.graph_title,
            "origin": self.origin,
            "access_token": self.access_token(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use rover_auth::external_token::ExternalTokenOrigin;
    use rover_client::{blocking::StudioCredentialOrigin, operations::config::who_am_i::Actor};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    #[fixture]
    fn output() -> ExternalIdentityOutput {
        ExternalIdentityOutput {
            identity: RegistryIdentity {
                id: "account-id".to_string(),
                graph_title: None,
                key_actor_type: Actor::USER,
                credential_origin: StudioCredentialOrigin::External(ExternalTokenOrigin::Option),
            },
            origin: "--access-token-file".to_string(),
            unmasked_token: None,
        }
    }

    #[rstest]
    fn external_identity_json_uses_the_standard_envelope(output: ExternalIdentityOutput) {
        let output = RoverOutput::CliOutput(Box::new(output));
        insta::assert_json_snapshot!(JsonOutput::from(&output));
    }

    #[rstest]
    fn external_identity_text_is_redacted(output: ExternalIdentityOutput) {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        insta::assert_snapshot!(text);
    }

    #[rstest]
    fn external_graph_identity_reports_its_graph_title(mut output: ExternalIdentityOutput) {
        output.identity.key_actor_type = Actor::GRAPH;
        output.identity.id = "graph-id".to_string();
        output.identity.graph_title = Some("Graph title".to_string());
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        insta::assert_snapshot!("external_graph_identity_text", text);
        let output = RoverOutput::CliOutput(Box::new(output));
        insta::assert_json_snapshot!("external_graph_identity_json", JsonOutput::from(&output));
    }

    #[rstest]
    fn external_identity_unmasking_does_not_expose_the_token_in_debug(
        mut output: ExternalIdentityOutput,
    ) {
        output.unmasked_token = Some(ExternalAccessToken::parse(b"private-credential").unwrap());
        assert_that!(format!("{output:?}")).is_equal_to(
            "ExternalIdentityOutput { identity: RegistryIdentity { id: \"account-id\", graph_title: None, key_actor_type: USER, credential_origin: External(Option) }, origin: \"--access-token-file\", unmasked_token: Some(ExternalAccessToken([REDACTED])) }".to_string()
        );
        assert_that!(output.access_token()).is_equal_to("private-credential");
    }
}
