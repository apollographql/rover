use core::fmt;
use std::{
    io,
    str::FromStr,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::Result;
use derive_getters::Getters;
use houston as config;
use reqwest::Client;
use rover_client::blocking::StudioClient;
use rover_http::{HttpService, ReqwestService, retry::retry_with_attempt_timeout};
use rover_studio::service::{HttpStudioServiceLayer, rejected_credential::RejectedCredentialLayer};
use serde::Serialize;
use tower::{ServiceBuilder, ServiceExt};
use url::Url;

use crate::{PKG_NAME, PKG_VERSION, options::ProfileOpt};

/// the Apollo graph registry's production API endpoint
const STUDIO_PROD_API_ENDPOINT: &str = "https://api.apollographql.com/graphql";

/// How long to wait to establish a connection when downloading a plugin tarball
/// before giving up. If the connection succesfully establishes we allow a much longer
/// period for the actual request.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const DOWNLOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Bounds a single plugin version lookup: a bodiless `HEAD` to the plugin registry. There's no
/// observed latency data for this request, so this is a conservative ceiling rather than a
/// measured one.
const PLUGIN_VERSION_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest a plugin version lookup keeps retrying. A failed lookup falls back to an
/// installed plugin where one exists, so an offline run shouldn't spend all of
/// `--client-timeout` retrying before it gets there. A shorter `--client-timeout` still wins.
const PLUGIN_VERSION_RETRY_BUDGET: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientBuilder {
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
    timeout: Option<std::time::Duration>,
    connect_timeout: Option<std::time::Duration>,
    follow_redirects: bool,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub const fn new() -> Self {
        Self {
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            timeout: None,
            connect_timeout: None,
            follow_redirects: true,
        }
    }

    pub const fn accept_invalid_certs(self, value: bool) -> Self {
        Self {
            accept_invalid_certs: value,
            ..self
        }
    }

    pub const fn accept_invalid_hostnames(self, value: bool) -> Self {
        Self {
            accept_invalid_hostnames: value,
            ..self
        }
    }

    pub const fn with_timeout(self, timeout: std::time::Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..self
        }
    }

    const fn clear_timeout(self) -> Self {
        Self {
            timeout: None,
            ..self
        }
    }

    const fn with_connect_timeout(self, connect_timeout: std::time::Duration) -> Self {
        Self {
            connect_timeout: Some(connect_timeout),
            ..self
        }
    }

    const fn without_redirects(self) -> Self {
        Self {
            follow_redirects: false,
            ..self
        }
    }

    pub(crate) fn build(self) -> Result<Client> {
        let mut builder = Client::builder()
            .gzip(true)
            .brotli(true)
            .danger_accept_invalid_certs(self.accept_invalid_certs)
            .danger_accept_invalid_hostnames(self.accept_invalid_hostnames);

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        if !self.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        let client = builder
            .user_agent(format!("{PKG_NAME}/{PKG_VERSION}"))
            .build()?;

        Ok(client)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub struct ClientTimeout {
    duration: Duration,
}

impl ClientTimeout {
    pub const fn new(duration_in_seconds: u64) -> ClientTimeout {
        ClientTimeout {
            duration: Duration::from_secs(duration_in_seconds),
        }
    }

    pub const fn get_duration(&self) -> Duration {
        self.duration
    }
}

impl Default for ClientTimeout {
    fn default() -> ClientTimeout {
        ClientTimeout::new(30)
    }
}

impl FromStr for ClientTimeout {
    type Err = io::Error;
    fn from_str(duration_in_secs: &str) -> std::result::Result<ClientTimeout, io::Error> {
        Ok(ClientTimeout::new(duration_in_secs.parse().map_err(
            |e| io::Error::new(io::ErrorKind::InvalidInput, e),
        )?))
    }
}

impl fmt::Display for ClientTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.duration.as_secs())
    }
}

impl From<Duration> for ClientTimeout {
    fn from(value: Duration) -> Self {
        ClientTimeout::new(value.as_secs())
    }
}

/// An override notice for a setting that only plugin downloads use - the
/// FR64 notice for a profile-resolved `APOLLO_ROVER_DOWNLOAD_HOST`, or the
/// FR59(a) notice for `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` - deferred from
/// resolution time (`Rover::get_client_config`, where the value and the
/// decision of whether to notice are both already settled) to the point a
/// plugin download actually happens (spec.md:403/FR60: a command that
/// downloads nothing must print nothing). `printed` is shared through
/// `StudioClientConfig`'s `Clone` via the surrounding `Arc`, so cloned
/// configs used across multiple plugin downloads in one process still only
/// print once (FR61).
#[derive(Debug)]
struct DeferredNotice {
    message: String,
    printed: OnceLock<()>,
}

impl DeferredNotice {
    fn new(message: String) -> Arc<Self> {
        Arc::new(Self {
            message,
            printed: OnceLock::new(),
        })
    }

    /// Prints the notice, unless it already printed.
    fn print_once(&self) {
        use rover_print::{print::Print, style::StyledText};

        self.printed.get_or_init(|| {
            rover_print::print::stderr::default()
                .print(&StyledText::plain(format!("Note: {}", self.message)));
        });
    }
}

#[derive(Debug, Clone, Getters)]
pub struct StudioClientConfig {
    #[getter(skip)]
    pub(crate) config: config::Config,
    client_builder: ClientBuilder,
    uri: String,
    version: String,
    is_sudo: bool,
    client: Option<Client>,
    client_timeout: ClientTimeout,
    download_timeout: Duration,
    download_host: Option<String>,
    #[getter(skip)]
    download_host_notice: Option<Arc<DeferredNotice>>,
    #[getter(skip)]
    allow_automatic_download: bool,
    #[getter(skip)]
    automatic_download_notice: Option<Arc<DeferredNotice>>,
}

impl StudioClientConfig {
    pub fn new(
        override_endpoint: Option<String>,
        config: config::Config,
        is_sudo: bool,
        client_builder: ClientBuilder,
        client_timeout: ClientTimeout,
    ) -> StudioClientConfig {
        let version = if cfg!(debug_assertions) {
            format!("{PKG_VERSION} (dev)")
        } else {
            PKG_VERSION.to_string()
        };

        StudioClientConfig {
            uri: override_endpoint.unwrap_or_else(|| STUDIO_PROD_API_ENDPOINT.to_string()),
            config,
            version,
            client_builder,
            is_sudo,
            client: None,
            client_timeout,
            download_timeout: DOWNLOAD_REQUEST_TIMEOUT,
            download_host: None,
            download_host_notice: None,
            allow_automatic_download: false,
            automatic_download_notice: None,
        }
    }

    pub const fn with_download_timeout(mut self, download_timeout: Duration) -> Self {
        self.download_timeout = download_timeout;
        self
    }

    /// Overrides the host plugin binaries (the `router`/`supergraph`
    /// composition plugins, and the MCP server binary) are downloaded from.
    /// Read by [`crate::command::install::plugin::Plugin::get_tarball_url`].
    pub fn with_download_host(mut self, download_host: String) -> Self {
        self.download_host = Some(download_host);
        self
    }

    /// Lets a command that needs a plugin installed nowhere download it on its
    /// own, as `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` resolved. Until this is
    /// called, it may not.
    pub const fn with_allow_automatic_download(mut self, allow: bool) -> Self {
        self.allow_automatic_download = allow;
        self
    }

    /// Whether a command may download a plugin it needs and doesn't have.
    pub const fn allow_automatic_download(&self) -> bool {
        self.allow_automatic_download
    }

    /// Carries the FR64 override notice text to print the first time a
    /// plugin download actually happens (see `DownloadHostNotice`), rather
    /// than unconditionally at resolution time.
    pub(crate) fn with_download_host_notice(mut self, message: String) -> Self {
        self.download_host_notice = Some(DeferredNotice::new(message));
        self
    }

    /// The download-host notice this config will print on its first
    /// download, if one was decided.
    #[cfg(test)]
    pub(crate) fn download_host_notice_message(&self) -> Option<&str> {
        self.download_host_notice
            .as_deref()
            .map(|notice| notice.message.as_str())
    }

    /// Prints the download-host override notice the first time this is
    /// called for a given resolved value (FR61: exactly one notice no
    /// matter how many artifacts one process downloads) - a no-op if
    /// there's no notice to print, or if it already printed.
    pub(crate) fn print_download_host_notice_once(&self) {
        if let Some(notice) = &self.download_host_notice {
            notice.print_once();
        }
    }

    /// Carries the FR59(a) notice for `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD`,
    /// to print the first time the opt-in lets a plugin download happen.
    pub(crate) fn with_automatic_download_notice(mut self, message: String) -> Self {
        self.automatic_download_notice = Some(DeferredNotice::new(message));
        self
    }

    /// The automatic-download notice this config will print when the opt-in
    /// first lets a download happen, if one was decided.
    #[cfg(test)]
    pub(crate) fn automatic_download_notice_message(&self) -> Option<&str> {
        self.automatic_download_notice
            .as_deref()
            .map(|notice| notice.message.as_str())
    }

    /// Prints the automatic-download notice, at most once per process (FR61).
    /// The commands that install plugins on the fly call this once the opt-in
    /// has let one download.
    pub(crate) fn print_automatic_download_notice_once(&self) {
        if let Some(notice) = &self.automatic_download_notice {
            notice.print_once();
        }
    }

    pub(crate) fn get_reqwest_client(&self) -> Result<Client> {
        if let Some(client) = &self.client {
            Ok(client.clone())
        } else {
            // we can use clone here freely since `reqwest` uses an `Arc` under the hood
            self.client_builder.build()
        }
    }

    pub fn service(&self) -> Result<HttpService> {
        let client = self.get_reqwest_client()?;
        Ok(ReqwestService::builder()
            .client(client)
            .build()?
            .boxed_clone())
    }

    /// A service for downloading large binaries (the `supergraph` and `router`
    /// plugins) with a much longer request timeout and a connection timeout to fail-fast
    /// in actual offline scenarios.
    pub fn download_service(&self) -> Result<HttpService> {
        let client = self
            .client_builder
            .clear_timeout()
            .with_timeout(self.download_timeout)
            .with_connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .build()?;
        Ok(ReqwestService::builder()
            .client(client)
            .build()?
            .boxed_clone())
    }

    /// A service for resolving a floating plugin version (`latest`, a bare major) against the
    /// plugin registry. It carries the global HTTP settings — `--client-timeout` and the
    /// certificate flags — and retries within [`Self::plugin_version_retry_budget`], bounding
    /// each attempt.
    ///
    /// It doesn't follow redirects: the registry answers with one, and the `X-Version` header
    /// on that redirect is the answer.
    pub fn plugin_version_service(&self) -> Result<HttpService> {
        let client = self.client_builder.without_redirects().build()?;
        Ok(ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                self.plugin_version_retry_budget(),
                PLUGIN_VERSION_ATTEMPT_TIMEOUT,
            ))
            .service(ReqwestService::builder().client(client).build()?)
            .boxed_clone())
    }

    pub fn get_authenticated_client(&self, profile_opt: &ProfileOpt) -> Result<StudioClient> {
        let credential =
            config::Profile::new(&profile_opt.profile_name, &self.config).get_credential()?;
        Ok(StudioClient::new(
            credential,
            &self.uri,
            &self.version,
            self.is_sudo,
            self.get_reqwest_client()?,
            self.client_timeout.get_duration(),
        ))
    }

    pub fn authenticated_service(&self, profile_opt: &ProfileOpt) -> Result<HttpService> {
        let client = self.get_reqwest_client()?;
        let credential =
            config::Profile::new(&profile_opt.profile_name, &self.config).get_credential()?;
        let service = ServiceBuilder::new()
            .layer(HttpStudioServiceLayer::new(
                Url::from_str(&self.uri)?,
                credential.clone(),
                self.version.clone(),
                self.is_sudo,
            )?)
            .layer(RejectedCredentialLayer::new(credential))
            .service(ReqwestService::builder().client(client).build()?)
            .boxed_clone();
        Ok(service)
    }

    pub const fn accept_invalid_certs(&self) -> bool {
        self.client_builder.accept_invalid_certs
    }

    pub const fn retry_period(&self) -> Duration {
        self.client_timeout.get_duration()
    }

    /// How long a plugin version lookup retries: `--client-timeout`, capped so an offline run
    /// reaches its installed-plugin fallback promptly.
    pub fn plugin_version_retry_budget(&self) -> Duration {
        self.retry_period().min(PLUGIN_VERSION_RETRY_BUDGET)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ClientBuilder;

    #[test]
    fn clear_timeout_drops_the_whole_request_deadline() {
        let builder = ClientBuilder::new().with_timeout(Duration::from_secs(30));
        assert_eq!(builder.timeout, Some(Duration::from_secs(30)));
        assert!(builder.clear_timeout().timeout.is_none());
    }

    #[test]
    fn with_connect_timeout_is_independent_of_the_request_timeout() {
        let builder = ClientBuilder::new()
            .with_timeout(Duration::from_secs(30))
            .with_connect_timeout(Duration::from_secs(5));
        assert_eq!(builder.timeout, Some(Duration::from_secs(30)));
        assert_eq!(builder.connect_timeout, Some(Duration::from_secs(5)));
    }

    fn test_client_config() -> super::StudioClientConfig {
        test_client_config_with_timeout(super::ClientTimeout::default())
    }

    fn test_client_config_with_timeout(
        client_timeout: super::ClientTimeout,
    ) -> super::StudioClientConfig {
        super::StudioClientConfig::new(
            None,
            houston::Config {
                home: camino::Utf8PathBuf::from("/tmp/rover-client-test"),
                override_api_key: None,
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default().with_timeout(client_timeout.get_duration()),
            client_timeout,
        )
    }

    fn head(uri: String) -> rover_http::HttpRequest {
        http::Request::head(uri)
            .body(rover_http::Full::default())
            .unwrap()
    }

    #[test]
    fn plugin_version_retry_budget_is_the_client_timeout_capped() {
        let default = test_client_config_with_timeout(super::ClientTimeout::new(30));
        assert_eq!(
            default.plugin_version_retry_budget(),
            super::PLUGIN_VERSION_RETRY_BUDGET
        );
        let shorter = test_client_config_with_timeout(super::ClientTimeout::new(2));
        assert_eq!(
            shorter.plugin_version_retry_budget(),
            Duration::from_secs(2)
        );
    }

    #[test]
    fn redirects_are_followed_unless_disabled() {
        assert!(ClientBuilder::new().follow_redirects);
        assert!(!ClientBuilder::new().without_redirects().follow_redirects);
    }

    /// The registry answers a floating version with a redirect whose `X-Version` header is the
    /// resolved version; following it would lose the header.
    #[tokio::test]
    async fn plugin_version_service_reads_the_registry_redirect_instead_of_following_it() {
        use tower::ServiceExt;

        let server = httpmock::MockServer::start();
        let redirect = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD)
                .path("/tar/supergraph/latest-2");
            then.status(302)
                .header("location", "/tar/supergraph/v2.9.3")
                .header("x-version", "v2.9.3");
        });
        let target = server.mock(|when, then| {
            when.path("/tar/supergraph/v2.9.3");
            then.status(200);
        });

        let response = test_client_config()
            .plugin_version_service()
            .unwrap()
            .oneshot(head(server.url("/tar/supergraph/latest-2")))
            .await
            .unwrap();

        redirect.assert_calls(1);
        target.assert_calls(0);
        assert_eq!(response.status(), http::StatusCode::FOUND);
        assert_eq!(response.headers()["x-version"], "v2.9.3");
    }

    /// A registry that keeps failing is retried until `--client-timeout` runs out, rather than
    /// failing the run on its first transient error.
    #[tokio::test]
    async fn plugin_version_service_retries_within_the_client_timeout() {
        use tower::ServiceExt;

        let server = httpmock::MockServer::start();
        let unavailable = server.mock(|when, then| {
            when.method(httpmock::Method::HEAD)
                .path("/tar/supergraph/latest-2");
            then.status(503);
        });

        let response = test_client_config_with_timeout(super::ClientTimeout::new(2))
            .plugin_version_service()
            .unwrap()
            .oneshot(head(server.url("/tar/supergraph/latest-2")))
            .await
            .unwrap();

        assert!(
            unavailable.calls() > 1,
            "expected a retry, got {} attempt(s)",
            unavailable.calls()
        );
        assert_eq!(response.status(), http::StatusCode::SERVICE_UNAVAILABLE);
    }

    /// Plugin downloads default to the generous timeout, and an explicit
    /// `--client-timeout` (applied via `with_download_timeout`) overrides it —
    /// up or down. Regression guard: #3358 made this unconfigurable.
    #[test]
    fn download_timeout_defaults_to_the_generous_default() {
        assert_eq!(
            *test_client_config().download_timeout(),
            super::DOWNLOAD_REQUEST_TIMEOUT
        );
    }

    #[test]
    fn with_download_timeout_overrides_the_default_up_or_down() {
        for secs in [5, 99_999] {
            let config = test_client_config().with_download_timeout(Duration::from_secs(secs));
            assert_eq!(*config.download_timeout(), Duration::from_secs(secs));
        }
    }

    // A command that never downloads a plugin never calls this at all
    // (spec.md:403/FR60) - confirms the no-notice-set case is a safe no-op,
    // not a panic on an absent `Option`.
    #[test]
    fn printing_with_no_download_host_notice_set_is_a_no_op() {
        test_client_config().print_download_host_notice_once();
    }

    #[test]
    fn the_download_host_notice_prints_at_most_once_even_across_clones() {
        use std::sync::Arc;

        let config = test_client_config().with_download_host_notice("test notice".to_string());
        let notice = Arc::clone(config.download_host_notice.as_ref().unwrap());
        assert!(notice.printed.get().is_none());

        config.print_download_host_notice_once();
        assert!(notice.printed.get().is_some());

        // A clone shares the same underlying notice (and its print-once
        // gate) through the surrounding `Arc`, not a fresh copy - a second
        // plugin download sharing this cloned config still only notices
        // once per process (FR61). The clone itself is the thing under
        // test, even though nothing below needs owned access.
        #[allow(clippy::redundant_clone)]
        let cloned = config.clone();
        assert!(Arc::ptr_eq(
            &notice,
            cloned.download_host_notice.as_ref().unwrap()
        ));
        cloned.print_download_host_notice_once();
    }
}
