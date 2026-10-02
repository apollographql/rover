use super::output::RevokeSweepOutput;
use crate::RoverErrorCode;

/// Failures of `rover auth grants revoke`'s per-user sweep that aren't the Platform API's own
/// (those stay `RoverClientError`s), per spec FR56, FR59, and FR63
/// (`specs/rover-431-identity-grant-management`).
#[derive(Debug, thiserror::Error)]
pub(crate) enum GrantsRevokeError {
    /// FR56: the pairs couldn't all be enumerated, so nothing was revoked.
    #[error(
        "Couldn't list organization `{organization_id}`'s client-credential pairs, so nothing \
        was revoked: {reason}"
    )]
    PairEnumeration {
        organization_id: String,
        reason: String,
    },

    /// FR59.
    #[error(
        "Revoking every grant for `{user_id}` needs confirmation, and there's no terminal to \
        ask on. Pass `--confirm` to proceed without a prompt."
    )]
    ConfirmationRequired { user_id: String },

    /// FR63: revocation failed under at least one client. Carries every client's outcome, for
    /// `data` (FR67) and the stderr summary.
    #[error(
        "Revocation failed under {} of {} OAuth clients.",
        .output.failed().count(),
        .output.clients.len()
    )]
    PartialFailure { output: RevokeSweepOutput },
}

impl GrantsRevokeError {
    /// FR74 #5 and #6. Enumeration failures have no dedicated code: FR74 doesn't name one.
    pub(crate) const fn code(&self) -> Option<RoverErrorCode> {
        match self {
            Self::ConfirmationRequired { .. } => Some(RoverErrorCode::E062),
            Self::PartialFailure { .. } => Some(RoverErrorCode::E063),
            Self::PairEnumeration { .. } => None,
        }
    }
}
