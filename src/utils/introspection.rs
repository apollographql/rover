use std::{collections::HashMap, str::FromStr, time::Duration};

use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue, Uri,
};
use rover_graphql::{GraphQLLayer, GraphQLServiceError};
use tower::{util::BoxCloneService, ServiceBuilder, ServiceExt};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IntrospectionResponse {
    result: String,
}

impl IntrospectionResponse {
    pub fn new(result: String) -> IntrospectionResponse {
        IntrospectionResponse { result }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IntrospectionServiceError {
    #[error(transparent)]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
}

pub enum RetryConfig {
    NoRetry,
    RetryWithDefault,
    RetryWith(Duration),
}

pub struct IntrospectionService {
    service: BoxCloneService<(), IntrospectionResponse, GraphQLServiceError<IntrospectionResponse>>,
}

impl IntrospectionService {
    pub fn new<S>(
        endpoint: Uri,
        headers: HashMap<String, String>,
        service: S,
    ) -> Result<IntrospectionService, IntrospectionServiceError> {
        let headers = headers
            .iter()
            .map(|(name, value)| {
                let header_name = HeaderName::from_str(name)?;
                let header_value = HeaderValue::from_str(value)?;
                Ok((header_name, header_value))
            })
            .collect::<Result<Vec<_>, IntrospectionServiceError>>()?;
        let headers = HeaderMap::from_iter(headers);
        let service = ServiceBuilder::new()
            .map_response(|resp: QueryResponseData)
            .layer(GraphQLLayer::new(endpoint))
            .map_request(move |mut req: http::Request<_>| {
                let req_headers = req.headers_mut();
                req_headers.extend(headers.clone());
                req
            })
            .service(service)
            .boxed_clone();
        IntrospectionService { service }
    }
}
