use anyhow::{anyhow, Result};
use clap::Parser;
use rover_auth::oauth2::register::{Register, RegisterRequest};
use rover_http::ReqwestService;
use tower::ServiceExt;
use url::Url;

/// RFC 8252 loopback redirect - the actual port is chosen at login time and
/// doesn't need to match what's registered here.
const REDIRECT_URL: &str = "http://127.0.0.1";

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OauthEnvironment {
    Dev0,
    Staging,
    Prod,
}

impl OauthEnvironment {
    const fn register_url(self) -> &'static str {
        match self {
            Self::Dev0 => "https://auth.dev.apollographql.com/oauth2/register",
            Self::Staging => "https://auth.staging.apollographql.com/oauth2/register",
            Self::Prod => "https://auth.apollographql.com/oauth2/register",
        }
    }
}

/// One-off provisioning: registers a static OAuth client for `rover auth
/// login` with an Identity environment's OAuth server and prints the
/// resulting client_id, per the `#proj-oauth` decision to use one static,
/// per-environment client_id rather than dynamic per-install registration.
#[derive(Debug, Parser)]
pub struct RegisterOauthClient {
    /// Which Identity environment to register a client with
    #[arg(long, value_enum)]
    env: OauthEnvironment,
}

impl RegisterOauthClient {
    pub async fn run(&self) -> Result<()> {
        let register_url = Url::parse(self.env.register_url())?;
        let redirect_url = Url::parse(REDIRECT_URL)?;

        let http_service = ReqwestService::builder()
            .client(reqwest::Client::new())
            .build()
            .map_err(|e| anyhow!("failed to build an HTTP client: {e}"))?;

        let request = RegisterRequest::builder()
            .register_url(register_url)
            .redirect_url(redirect_url)
            .build();

        let response = Register::new(http_service)
            .oneshot(request)
            .await
            .map_err(|e| anyhow!("registration request failed: {e}"))?;

        println!("{}", response.client_id);
        Ok(())
    }
}
