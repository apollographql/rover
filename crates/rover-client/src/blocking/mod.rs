mod client;
mod studio_client;
#[cfg(test)]
mod trace_recorder;

pub use client::GraphQLClient;
pub use studio_client::StudioClient;

pub(crate) const CLIENT_NAME: &str = "rover-client";
