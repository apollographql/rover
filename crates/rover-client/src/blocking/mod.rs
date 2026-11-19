mod client;
mod studio_client;

pub use client::GraphQLClient;
pub use studio_client::{StudioClient, StudioCredentialOrigin};

pub(crate) const CLIENT_NAME: &str = "rover-client";
