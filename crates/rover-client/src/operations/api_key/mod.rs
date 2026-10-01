pub mod create;
pub mod delete;
pub mod get;
pub mod list;
pub mod pair_create;
pub mod pair_delete;
pub mod pair_get;
pub mod pair_list;
pub mod pair_rotate;
pub mod rename;

pub use crate::operations::api_key::create::create_key_mutation::GraphOsKeyType;
use crate::RoverClientError;

/// `client_secret`/`secret_expires_at` are nullable at the schema level (non-null only
/// alongside a freshly minted secret, per `OAuthClient`'s doc comment) but always present on a
/// successful create/rotate response - the absence of either one here means something is
/// actually wrong, not a legitimate empty state. Shared by `pair_create` and `pair_rotate` so
/// the wording can't drift between the two.
pub(crate) fn missing_secret_data() -> RoverClientError {
    RoverClientError::ClientError {
        msg: "the Platform API did not return the pair's new secret or its expiry".to_string(),
    }
}
