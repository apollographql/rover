use std::time::Duration;

use bon::bon;
use bytes::Bytes;
use reqwest::header::{self, HeaderMap, HeaderValue};
use rover_http::{
    extend_headers::ExtendHeadersLayer, retry::RetryPolicy, timeout::TimeoutLayer, Full,
    HttpRequest, HttpResponse, HttpServiceError,
};
use tower::{retry::RetryLayer, util::BoxService, Service, ServiceBuilder};
use tower_http::decompression::{DecompressionBody, DecompressionLayer};

pub mod gz_decode;

const DEFAULT_ELAPSED_DURATION_SECONDS: u64 = 600;
const DEFAULT_TIMEOUT_DURATION_SECONDS: u64 = 60;
const ROVER_CLIENT_HEADER: HeaderValue = HeaderValue::from_static("rover-client");
const OCTET_STREAM_HEADER: HeaderValue = HeaderValue::from_static("application/octet-stream");

pub struct FileDownloadService {
    inner: BoxService<HttpRequest, HttpResponse<DecompressionBody<Full<Bytes>>>, HttpServiceError>,
}

#[bon]
impl FileDownloadService {
    #[builder]
    pub fn new<S1>(
        http_service: S1,
        max_elapsed_duration: Option<Duration>,
        timeout_duration: Option<Duration>,
    ) -> FileDownloadService
    where
        S1: Service<HttpRequest, Response = HttpResponse> + Clone + Send + 'static,
        S1::Error: Into<HttpServiceError>,
        S1::Future: Send + 'static,
    {
        let service = ServiceBuilder::new()
            .boxed()
            .layer(DecompressionLayer::default()) // explicit stand-in for reqwest's brotli/gzip decompression options
            .layer(file_download_layer())
            .layer(RetryLayer::new(RetryPolicy::new(
                max_elapsed_duration
                    .unwrap_or_else(|| Duration::from_secs(DEFAULT_ELAPSED_DURATION_SECONDS)),
            )))
            .layer(TimeoutLayer::new(timeout_duration.unwrap_or_else(|| {
                Duration::from_secs(DEFAULT_TIMEOUT_DURATION_SECONDS)
            })))
            .service(http_service);
        FileDownloadService { inner: service }
    }

    pub fn into_inner(
        self,
    ) -> BoxService<HttpRequest, HttpResponse<DecompressionBody<Full<Bytes>>>, HttpServiceError>
    {
        self.inner
    }
}

pub fn file_download_layer() -> ExtendHeadersLayer {
    ExtendHeadersLayer::new(HeaderMap::from_iter([
        (header::USER_AGENT, ROVER_CLIENT_HEADER),
        (header::ACCEPT, OCTET_STREAM_HEADER),
    ]))
}
