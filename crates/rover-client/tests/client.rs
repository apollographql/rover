use reqwest::blocking::Client;
const STUDIO_PROD_API_ENDPOINT: &str = "https://graphql.api.apollographql.com/api/graphql";

pub(crate) fn get_client() -> Client {
    Client::builder()
        .gzip(true)
        .brotli(true)
        .build()
        .expect("Could not create reqwest Client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston::{Credential, CredentialOrigin};
    use rover_client::blocking::{GraphQLClient, StudioClient};

    use crate::STUDIO_PROD_API_ENDPOINT;

    #[test]
    fn it_can_build_client() {
        assert!(GraphQLClient::new(STUDIO_PROD_API_ENDPOINT, get_client()).is_ok(),);
    }

    #[test]
    fn it_can_build_studio_client() {
        assert!(StudioClient::new(
            Credential {
                api_key: "api:key:here".to_string(),
                origin: CredentialOrigin::EnvVar,
            },
            "0.1.0",
            STUDIO_PROD_API_ENDPOINT,
            get_client()
        )
        .is_ok());
    }
}
