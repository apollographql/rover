use anyhow::anyhow;
use clap::Parser;
use rover_oauth::{DeviceFlowClient, OAuthClientConfig, OAuthTokens};
use rover_std::Style;
use serde::Serialize;

use config::Profile;
use houston as config;

use crate::{options::ProfileOpt, RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
/// Authenticate using OAuth 2.1 Device Code Flow
///
/// This command starts an OAuth 2.1 Device Code Flow to authenticate
/// with Apollo Studio. It will open your browser and prompt you to
/// authorize Rover.
///
/// Running this command with a --profile <name> argument will create a new
/// profile that can be referenced by name across Rover with the --profile
/// <name> argument.
///
/// Running without the --profile flag will set OAuth tokens for
/// a profile named "default".
pub struct OAuth {
    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(long, help = "Apollo Studio URL")]
    studio_url: Option<String>,

    #[clap(long, help = "OAuth client ID (optional, will auto-register if not provided)")]
    client_id: Option<String>,

    #[clap(long, help = "OAuth scopes to request")]
    scopes: Option<Vec<String>>,
}

impl OAuth {
    pub async fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        let oauth_config = OAuthClientConfig {
            client_id: self.client_id.clone(),
            client_secret: None, // Public client, no secret needed
            authorization_server_url: self.studio_url.clone()
                .unwrap_or_else(|| "http://localhost:3000".to_string()),
            scopes: self.scopes.clone()
                .or_else(|| Some(vec!["rover".to_string()])),
            redirect_uri: None, // Not used in device flow
        };

        let mut client = DeviceFlowClient::new(oauth_config)
            .map_err(|e| anyhow!("Failed to create OAuth client: {}", e))?;

        eprintln!(
            "🔐 Starting OAuth authentication for profile {}...",
            Style::Command.paint(&self.profile.profile_name)
        );

        match client.authenticate().await {
            Ok(tokens) => {
                // Store tokens in the profile
                self.store_oauth_tokens(&config, &tokens)?;
                
                eprintln!(
                    "✅ Successfully authenticated and saved OAuth tokens for profile {}",
                    Style::Command.paint(&self.profile.profile_name)
                );
                eprintln!("Consider running `rover config whoami` to verify your authentication.");
                
                Ok(RoverOutput::EmptySuccess)
            }
            Err(rover_oauth::OAuthError::AccessDenied) => {
                Err(anyhow!("Authentication was denied. Please try again and authorize Rover when prompted.").into())
            }
            Err(rover_oauth::OAuthError::Timeout) => {
                Err(anyhow!("Authentication timed out. Please try again and complete the authorization more quickly.").into())
            }
            Err(rover_oauth::OAuthError::BrowserError(msg)) => {
                let mut err = RoverError::new(anyhow!("Failed to open browser: {}", msg));
                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                    "Please manually navigate to the authorization URL displayed above.".to_string()
                ));
                Err(err)
            }
            Err(e) => {
                Err(anyhow!("OAuth authentication failed: {}", e).into())
            }
        }
    }

    fn store_oauth_tokens(&self, config: &config::Config, tokens: &OAuthTokens) -> RoverResult<()> {
        // For now, we'll store the access token as an API key
        // In a full implementation, we'd extend the Profile system to handle OAuth tokens
        Profile::set_api_key(&self.profile.profile_name, config, &tokens.access_token)
            .map_err(|e| anyhow!("Failed to store OAuth tokens: {}", e))?;

        // TODO: Store additional OAuth metadata (refresh token, expiration, etc.)
        // This would require extending the houston crate's Profile system

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8Path;

    fn get_test_config() -> config::Config {
        let tmp_home = TempDir::new().unwrap();
        let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
        config::Config::new(Some(&tmp_home_path), None).unwrap()
    }

    #[test]
    fn test_oauth_command_creation() {
        let oauth = OAuth {
            profile: ProfileOpt {
                profile_name: "test".to_string(),
            },
            studio_url: None,
            client_id: None,
            scopes: None,
        };

        assert_eq!(oauth.profile.profile_name, "test");
    }
}