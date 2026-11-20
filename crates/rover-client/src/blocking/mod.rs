mod client;
mod external;
mod studio_client;

pub use client::GraphQLClient;
pub use external::ExternalTokenCapture;
pub use studio_client::StudioClient;

pub(crate) const CLIENT_NAME: &str = "rover-client";
