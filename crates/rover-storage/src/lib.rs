mod credentials_file;
pub mod secret;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("No keystore backend available")]
    NoBackend,
    #[error("Failed to serialize")]
    Serialize(#[source] serde_json::Error),
    #[error("Failed to deserialize")]
    Deserialize(#[source] serde_json::Error),
    #[error(
        "wrote a secret to the credential store, but a follow-up read could not confirm it was saved"
    )]
    WriteNotVisible,
    #[error(transparent)]
    Store(Box<dyn std::error::Error + Send + Sync>),
}

impl From<keyring_core::Error> for StoreError {
    fn from(err: keyring_core::Error) -> Self {
        match err {
            keyring_core::Error::NoStorageAccess(_) => StoreError::NoBackend,
            err => StoreError::Store(Box::new(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn serialize_message_excludes_its_cause() {
        let inner = serde_json::from_str::<()>("not json").unwrap_err();
        let cause = inner.to_string();
        let err = StoreError::Serialize(inner);

        assert_that!(err.to_string()).is_equal_to("Failed to serialize".to_string());
        assert_that!(err.source().expect("cause is still linked").to_string()).is_equal_to(cause);
    }

    #[test]
    fn deserialize_message_excludes_its_cause() {
        let inner = serde_json::from_str::<()>("not json").unwrap_err();
        let cause = inner.to_string();
        let err = StoreError::Deserialize(inner);

        assert_that!(err.to_string()).is_equal_to("Failed to deserialize".to_string());
        assert_that!(err.source().expect("cause is still linked").to_string()).is_equal_to(cause);
    }

    /// `Store` is `#[error(transparent)]`, so it forwards both its message and its source to the
    /// error it wraps rather than adding a layer of its own.
    #[test]
    fn store_forwards_the_message_of_the_error_it_wraps() {
        let err = StoreError::Store(Box::<dyn Error + Send + Sync>::from(
            "the keystore refused the write",
        ));

        assert_that!(err.to_string()).is_equal_to("the keystore refused the write".to_string());
    }
}
