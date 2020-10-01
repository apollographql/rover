use crate::RoverClientError;
use std::env;

const CONTENT_TYPE: &str = "application/json";
const CLIENT_NAME: &str = "rover-client";

/// Function for building a [HeaderMap] for making http requests.
///
/// Takes a single argument, "api_key"m and returns a [HeaderMap].
pub fn build(api_key: &str) -> Result<reqwest::header::HeaderMap, RoverClientError> {
    let mut headers = reqwest::header::HeaderMap::new();

    let content_type =
        reqwest::header::HeaderValue::from_str(CONTENT_TYPE).expect("is valid header");
    headers.insert("Content-Type", content_type);

    let client_name = reqwest::header::HeaderValue::from_str(CLIENT_NAME).expect("is valid header");
    headers.insert("apollographql-client-name", client_name);

    let client_version = reqwest::header::HeaderValue::from_str(
        &env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| String::from("unknown")),
    )
    .expect("is valid header");
    headers.insert("apollographql-client-version", client_version);

    let mut api_key = reqwest::header::HeaderValue::from_str(api_key)
        .map_err(|e| RoverClientError::HeadersError { msg: e })?;
    api_key.set_sensitive(true);
    headers.insert("x-api-key", api_key);

    Ok(headers)
}
