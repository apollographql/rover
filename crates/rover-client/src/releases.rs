use crate::{error::EndpointKind, RoverClientError};

use reqwest::Client;
pub use semver::Version;

const LATEST_RELEASE_URL: &str = "https://github.com/apollographql/rover/releases/latest";

/// Looks up and parses the latest release version
pub async fn get_latest_release(client: Client) -> Result<Version, RoverClientError> {
    // send a request to the latest GitHub release
    let response = client.head(LATEST_RELEASE_URL).send().await.map_err(|e| {
        RoverClientError::SendRequest {
            source: e,
            endpoint_kind: EndpointKind::Orbiter,
        }
    })?;

    // this will return a response with a redirect to the latest tagged release
    let url_path_segments = response
        .url()
        .path_segments()
        .ok_or(RoverClientError::BadReleaseUrl)?;

    // the last section of the URL will have the latest version in `v0.1.1` format
    let version_string = url_path_segments
        .last()
        .ok_or(RoverClientError::BadReleaseUrl)?
        .to_string();

    // strip the `v` prefix from the last section of the URL before parsing
    Version::parse(&version_string[1..])
        .map_err(|source| RoverClientError::UnparseableReleaseVersion { source })
}
