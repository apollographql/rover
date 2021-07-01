use crate::{blocking::get_client, RoverClientError};
use regex::Regex;

const LATEST_RELEASE_URL: &str = "https://github.com/apollographql/rover/releases/latest";

/// Looks up the latest release version, and returns it as a string
pub fn get_latest_release() -> Result<String, RoverClientError> {
    let res = get_client()?.head(LATEST_RELEASE_URL).send()?;

    let release_url = res.url().to_string();
    let release_url_parts: Vec<&str> = release_url.split('/').collect();

    match release_url_parts.last() {
        Some(version) => {
            // Parse out the semver version (ex. v1.0.0 -> 1.0.0)
            let re = Regex::new(r"^v[0-9]*\.[0-9]*\.[0-9]*$").unwrap();
            if re.is_match(version) {
                Ok(version.to_string().replace('v', ""))
            } else {
                Err(RoverClientError::UnparseableReleaseVersion)
            }
        }
        None => Err(RoverClientError::UnparseableReleaseVersion),
    }
}
