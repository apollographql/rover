mod studio_client;

pub use introspector_gadget::blocking::GraphQLClient;
pub use studio_client::StudioClient;

pub(crate) const CLIENT_NAME: &str = "rover-client";
