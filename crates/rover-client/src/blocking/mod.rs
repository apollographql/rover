mod studio_client;

pub use launchpad::blocking::GraphQLClient;
pub use studio_client::StudioClient;

pub(crate) const CLIENT_NAME: &str = "rover-client";
