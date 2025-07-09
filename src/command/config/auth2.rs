use clap::Parser;
use rand::{distr::Alphanumeric, Rng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{ process::Command };
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

// server stuff
use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, Method };
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use reqwest::Client;



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
pub struct Auth2 {
    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Auth2{
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        eprintln!("Running new auth command");

        let (_, challenge) = Self::generate_verifier_and_encoded_hash();
        
        // Define the URL to open
        let redirect_uri = "http://localhost:3000/callback";
        let client_id = "your_client_id";
        let authorize_url = "http://localhost:8080/authorize"; // Replace with your actual auth server URL

        let url = format!("{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256", authorize_url, client_id, redirect_uri, challenge);

        // Attempt to open the URL in the default web browser
        let result = Command::new("open")
            .arg(url.clone())
            .status();

        match result {
            Ok(status) if status.success() => {
                eprintln!("Opened browser to URL: {}", url);
            }
            _ => {
                eprintln!("Failed to open browser to URL: {}", url);
            }
        }

        Self::start_server().await?;
        
        Ok(RoverOutput::EmptySuccess)
    }

    fn generate_verifier_and_encoded_hash() -> (String, String) {
        // Generate a random verifier (e.g., 128 characters long)
        // let verifier: String = rand::rng()
        //     .sample_iter(&Alphanumeric)
        //     .take(128) // Length of the verifier
        //     .map(char::from)
        //     .collect();
        let verifier = "hello".to_string();
    
        // Compute the SHA-256 hash of the verifier
        let digest = Sha256::digest(verifier.as_bytes());
    
        // Encode the hash in Base64 (URL-safe, without padding)
        let challenge = URL_SAFE_NO_PAD.encode(digest);
    
        (verifier, challenge)
    }  

    async fn hello(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
        if req.method() == Method::GET && req.uri().path() == "/callback" {
            // Handle the `/callback` route
            if let Some(query) = req.uri().query() {
                // Parse the `code` parameter from the query string
                if let Some(code) = query.split("code=").nth(1).and_then(|s| s.split('&').next()) {
                    println!("Received auth code: {}", code);
    
                    // Prepare the token request
                    let redirect_uri = "http://localhost:3000/callback";
                    let client_id = "your_client_id";
                    let verifier = "hello"; // Replace with the actual verifier
    
                    let token_endpoint = "http://localhost:8080/token";
                    let params = format!(
                        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
                        code, redirect_uri, client_id, verifier
                    );
    
                    let client = Client::new();
                    match client
                        .post(token_endpoint)
                        .header("Content-Type", "application/x-www-form-urlencoded")
                        .header("Accept", "application/json")
                        .body(params)
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() {
                                println!("Token request successful!");
                                let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
                                println!("Response body: {}", body);
                                return Ok(Response::new(Full::from(Bytes::from("Token request completed!"))));
                            } else {
                                println!("Token request failed with status: {}", response.status());
                                return Ok(Response::new(Full::from(Bytes::from("Token request failed!"))));
                            }
                            
                        }
                        Err(err) => {
                            eprintln!("Error sending token request: {:?}", err);
                            return Ok(Response::new(Full::from(Bytes::from("Token request failed!"))));
                        }
                    }
                }
            }
            Ok(Response::new(Full::from(Bytes::from("Invalid callback request"))))
        } else {
            // Default response for other routes
            Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
        }
    }

    async fn start_server() -> Result<(), anyhow::Error> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        // We create a TcpListener and bind it to 127.0.0.1:3000
        let listener = TcpListener::bind(addr).await?;

        // We start a loop to continuously accept incoming connections
        loop {
            let (stream, _) = listener.accept().await?;

            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);

            // Spawn a tokio task to serve multiple connections concurrently
            tokio::task::spawn(async move {
                // Finally, we bind the incoming connection to our `hello` service
                if let Err(err) = http1::Builder::new()
                    // `service_fn` converts our function in a `Service`
                    .serve_connection(io, service_fn(Auth2::hello))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

