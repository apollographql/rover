use crate::RoverClientError;
use reqwest::header::{HeaderMap, HeaderValue};
use std::env;

const CONTENT_TYPE: &str = "application/json";
const CLIENT_NAME: &str = "rover-client";

/// Function for building a [HeaderMap] for making http requests.
///
/// Takes a single argument, "api_key"m and returns a [HeaderMap].
pub fn build(api_key: &str) -> Result<HeaderMap, RoverClientError> {
    let mut headers = HeaderMap::new();

    let content_type = HeaderValue::from_str(CONTENT_TYPE).expect("is valid header");
    headers.insert("Content-Type", content_type);

    // this header value is used for client identification in Apollo Studio, so
    // the metrics in Studio can help us keep track of what parts of the schema
    // Rover uses and make sure we don't accidentally break those :)
    // more here: https://www.apollographql.com/docs/studio/client-awareness/#using-apollo-server-and-apollo-client
    let client_name = HeaderValue::from_str(CLIENT_NAME).expect("is valid header");
    headers.insert("apollographql-client-name", client_name);

    // see note above
    let client_version = HeaderValue::from_str(
        &env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| String::from("unknown")),
    )
    .expect("is valid header");
    headers.insert("apollographql-client-version", client_version);

    let mut api_key =
        HeaderValue::from_str(api_key).map_err(|e| RoverClientError::HeadersError { msg: e })?;
    api_key.set_sensitive(true);
    headers.insert("x-api-key", api_key);

    Ok(headers)
}
