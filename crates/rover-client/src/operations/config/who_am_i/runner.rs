use tower::{Service, ServiceExt};

use super::service::{WhoAmI, WhoAmIRequest};
use crate::{
    blocking::StudioClient, operations::config::who_am_i::types::RegistryIdentity, RoverClientError,
};

/// Get info from the registry about an API key, i.e. the name/id of the
/// user/graph and what kind of key it is (GRAPH/USER/Other)
pub async fn run(client: &StudioClient) -> Result<RegistryIdentity, RoverClientError> {
    let mut service = WhoAmI::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let identity = service
        .call(WhoAmIRequest::new(client.get_credential_origin()))
        .await
        .map_err(RoverClientError::from)
        .map_err(|err| client.refine_rejected_credential_error(err))?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use rstest::rstest;
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;

    fn client_with(server: &MockServer, api_key: &str) -> StudioClient {
        StudioClient::new(
            Credential {
                api_key: api_key.to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            &server.url("/"),
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        )
    }

    /// The registry rejects a credential at the gateway with HTTP 200, `"data": null` and a
    /// body-level "Invalid credentials" error, which on its own can only be reported as
    /// `InvalidKey`. `run` refines that against the key's own shape, so a key that could never
    /// have been valid is reported as malformed (E014) rather than unrecognized (E013).
    #[rstest]
    #[case::malformed_key("notakey", true)]
    #[case::well_formed_key("user:my-username:secretkey", false)]
    #[tokio::test]
    async fn a_body_level_rejection_is_malformed_only_when_the_key_shape_is_wrong(
        #[case] api_key: &str,
        #[case] malformed: bool,
    ) {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).body_includes("ConfigWhoAmIQuery");
            then.status(200).json_body(json!({
                "data": null,
                "errors": [{ "message": "Unauthorized: Invalid credentials provided" }]
            }));
        });

        let result = run(&client_with(&server, api_key)).await;

        mock.assert();
        assert_that!(result.is_err_and(|err| {
            if malformed {
                matches!(err, RoverClientError::MalformedKey)
            } else {
                matches!(err, RoverClientError::InvalidKey)
            }
        }))
        .is_true();
    }
}
