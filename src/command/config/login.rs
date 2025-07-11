use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use clap::Parser;
use rand::{distr::Alphanumeric, Rng};
use rover_std::Style;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::process::Command;
use tokio::sync::oneshot;

use config::Profile;
use houston as config;

// server stuff
use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use reqwest::Client;
use tokio::net::TcpListener;

// response handling
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{options::ProfileOpt, RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
/// Authenticate a configuration profile with an API key
///
/// Running this command with a --profile <name> argument will create a new
/// profile that can be referenced by name across Rover with the --profile
/// <name> argument.
///
/// Running without the --profile flag will set an API key for
/// a profile named "default".
///
/// Run `rover docs open api-keys` for more details on Apollo's API keys.
pub struct Login {
    #[clap(flatten)]
    profile: ProfileOpt,
}

#[derive(Clone)]
struct AuthConfig {
    redirect_uri: String,
    client_id: String,
    authorize_url: String,
    verifier: String,
    challenge: String,
    token_url: String,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    // Define fields based on the expected JSON structure
    access_token: Option<String>,
    expires_in: Option<u64>,
}

impl Login {
    pub async fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        let (verifier, challenge) = Self::generate_verifier_and_encoded_hash();

        let auth_config = AuthConfig {
            redirect_uri: "http://localhost:3000/callback".to_string(),
            client_id: "ouQM8NFUFZEXLsF3Wyw7WVqSd3pFyiVRUpaXLo9DjSc".to_string(),
            authorize_url: "https://graphql-staging.api.apollographql.com/auth/oauth2/authorize".to_string(),
            verifier: verifier.clone(),
            challenge: challenge.clone(),
            token_url: "https://graphql-staging.api.apollographql.com/auth/oauth2/token".to_string(),
        };

        //let login_url = "https://apollo-auth.netlify.app/login?redirect=".to_string();
        let login_url = "";

        let url = format!("{}{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256", login_url, auth_config.authorize_url, auth_config.client_id, auth_config.redirect_uri, auth_config.challenge);

        // Attempt to open the URL in the default web browser
        let result = Command::new("open").arg(url.clone()).status();

        match result {
            Ok(status) if status.success() => {
                println!("Opened browser to URL: {url}");
            }
            _ => {
                eprintln!("Failed to open browser to URL: {url}");
            }
        }

        Self::start_server(self, auth_config, config).await?;

        Ok(RoverOutput::EmptySuccess)
    }

    fn generate_verifier_and_encoded_hash() -> (String, String) {
        //Generate a random verifier (e.g., 128 characters long)
        let verifier: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(128) // Length of the verifier
            .map(char::from)
            .collect();

        // Compute the SHA-256 hash of the verifier
        let digest = Sha256::digest(verifier.as_bytes());

        // Encode the hash in Base64 (URL-safe, without padding)
        let challenge = URL_SAFE_NO_PAD.encode(digest);

        (verifier, challenge)
    }

    async fn handle_callback(
        &self,
        req: Request<hyper::body::Incoming>,
        auth_config: AuthConfig,
        config: config::Config,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        if req.method() == Method::GET && req.uri().path() == "/callback" {
            // Handle the `/callback` route
            if let Some(query) = req.uri().query() {
                // Parse the `code` parameter from the query string
                if let Some(code) = query
                    .split("code=")
                    .nth(1)
                    .and_then(|s| s.split('&').next())
                {
                    println!("Received auth code: {code}");
                    // Prepare the token request parameters
                    let params = format!(
                        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
                        code, auth_config.redirect_uri.clone(), auth_config.client_id.clone(), auth_config.verifier.clone()
                    );

                    let client = Client::new();
                    match client
                        .post(auth_config.token_url.clone())
                        .header("Content-Type", "application/x-www-form-urlencoded")
                        .header("Accept", "application/json")
                        .body(params)
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() {
                                let _ = Self::handle_response_body(self, response, config).await;
                                println!("{}", Style::Success.paint("Authentication successful!"));

                                // Redirect to a success page
                                let success_url =
                                    "https://apollo-auth.netlify.app/status?code=success"; // Replace with your desired URL
                                let response = Response::builder()
                                    .status(302)
                                    .header("Location", success_url)
                                    .body(Full::from(Bytes::new()))
                                    .unwrap();
                                return Ok(response);
                            } else {
                                println!(
                                    "Authentication request failed with status: {}",
                                    response.status()
                                );
                                return Ok(Response::new(Full::from(Bytes::from(
                                    "Authentication request failed!",
                                ))));
                            }
                        }
                        Err(err) => {
                            eprintln!("Error sending token request: {err:?}");
                            return Ok(Response::new(Full::from(Bytes::from(
                                "Token request failed!",
                            ))));
                        }
                    }
                }
            }
            Ok(Response::new(Full::from(Bytes::from(
                "Invalid callback request",
            ))))
        } else {
            // Default response for other routes
            Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
        }
    }

    async fn handle_response_body(&self, response: reqwest::Response, config: config::Config) {
        match response.json::<ResponseData>().await {
            Ok(json) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_else(|_| std::time::Duration::new(0, 0))
                    .as_secs();
                println!(
                    "Access Token: {}",
                    json.access_token
                        .as_deref()
                        .unwrap_or("No access token received")
                );
                let _ = Profile::set_access_token(
                    &self.profile.profile_name,
                    &config,
                    json.access_token.unwrap_or_default(),
                    now + json.expires_in.unwrap_or(0),
                );
                let _ = Profile::get_credential(&self.profile.profile_name, &config);
            }
            Err(err) => {
                eprintln!("Failed to parse response as JSON: {err:?}");
            }
        }
    }

    async fn start_server(
        &self,
        auth_config: AuthConfig,
        config: config::Config,
    ) -> Result<(), anyhow::Error> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = TcpListener::bind(addr).await?;
        println!("Server running on http://{addr}");

        // Create a shutdown signal
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // create and start connection
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let server = http1::Builder::new().serve_connection(
            io,
            service_fn(|req| {
                Login::handle_callback(self, req, auth_config.clone(), config.clone())
            }),
        );

        let server_result = tokio::select! {
            result = server => {
                result
            }
            _ = async {
                shutdown_rx.await.ok(); // Wait for the shutdown signal
            } => {
                Ok(())
            }
        };

        if let Err(err) = server_result {
            eprintln!("Error serving connection: {err:?}");
        }

        println!("Authentication complete. You can close this window now.");

        // Send the shutdown signal
        let _ = shutdown_tx.send(());

        Ok(())
    }
}
