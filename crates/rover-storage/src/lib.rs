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
    use speculoos::prelude::*;

    use super::*;

    /// How many times `marker` shows up in the way `RoverError`/`anyhow` actually render an
    /// error: `{:?}` on the wrapping `anyhow::Error`, which appends a "Caused by:" section for
    /// each link in the source chain. A variant that both embeds its cause's text inline and
    /// registers that cause as its `source` would show `marker` here twice.
    fn caused_by_count(err: StoreError, marker: &str) -> usize {
        format!("{:?}", anyhow::Error::new(err))
            .matches(marker)
            .count()
    }

    #[test]
    fn serialize_cause_is_not_duplicated() {
        let inner = serde_json::from_str::<()>("not json").unwrap_err();
        let marker = inner.to_string();

        assert_that!(caused_by_count(StoreError::Serialize(inner), &marker)).is_equal_to(1);
    }

    #[test]
    fn deserialize_cause_is_not_duplicated() {
        let inner = serde_json::from_str::<()>("not json").unwrap_err();
        let marker = inner.to_string();

        assert_that!(caused_by_count(StoreError::Deserialize(inner), &marker)).is_equal_to(1);
    }

    #[test]
    fn store_cause_is_not_duplicated() {
        let inner = Box::<dyn std::error::Error + Send + Sync>::from("store-marker");

        assert_that!(caused_by_count(StoreError::Store(inner), "store-marker")).is_equal_to(1);
    }
}
