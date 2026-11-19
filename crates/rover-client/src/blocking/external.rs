//! Opt-in observation of the external token an identity lookup authenticated with.

use std::sync::{Arc, Mutex};

use rover_auth::external_token::ExternalAccessToken;

/// Opt-in, short-lived capture for deliberate identity credential unmasking.
///
/// Use a fresh capture for each identity lookup, and inspect it only after that lookup
/// succeeds. It observes reads made by authentication, never supplies credentials to it,
/// and does not reread the file after the server has answered.
#[derive(Clone, Default)]
pub struct ExternalTokenCapture(Arc<Mutex<Option<ExternalAccessToken>>>);

impl std::fmt::Debug for ExternalTokenCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExternalTokenCapture([REDACTED])")
    }
}

impl ExternalTokenCapture {
    /// Removes the token used by the lookup's final successful attempt.
    ///
    /// A failed lookup must not expose the captured value.
    pub fn take(&self) -> Option<ExternalAccessToken> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub(super) fn observe(&self, token: ExternalAccessToken) -> ExternalAccessToken {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.clone());
        token
    }
}
