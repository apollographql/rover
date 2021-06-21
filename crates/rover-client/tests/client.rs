const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

#[cfg(test)]
mod tests {
    use houston::{Credential, CredentialOrigin};
    use rover_client::blocking::{GraphQLClient, StudioClient};

    use crate::STUDIO_PROD_API_ENDPOINT;

    #[test]
    fn it_can_build_client() {
        assert!(GraphQLClient::new(STUDIO_PROD_API_ENDPOINT).is_ok());
    }

    #[test]
    fn it_can_build_studio_client() {
        assert!(StudioClient::new(
            Credential {
                api_key: "api:key:here".to_string(),
                origin: CredentialOrigin::EnvVar,
            },
            "0.1.0",
            STUDIO_PROD_API_ENDPOINT
        )
        .is_ok());
    }
}
