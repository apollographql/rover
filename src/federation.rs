//! Shared Federation-1-rejection logic used by both composition and the `install --plugin`
//! command. Lives at the crate root (rather than under `composition` or `command::install`) so
//! neither of those modules has to depend on the other to reach it.

use apollo_federation_types::config::FederationVersion;

/// Documentation link describing how to opt a subgraph in to Federation 2 via `@link`.
pub const FEDERATION_2_MIGRATION_URL: &str = "https://www.apollographql.com/docs/federation/federation-2/moving-to-federation-2#opt-in-to-federation-2";

/// Error returned whenever a resolved [`FederationVersion`] is Federation 1. Rover no longer
/// supports composing, installing, or building against Federation 1 in any form.
///
/// This is shared verbatim between every entry point that can produce a `FederationVersion`
/// (composition, `rover install --plugin`), so the rejection message stays consistent.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "Federation 1 is no longer supported by Rover. Migrate your subgraphs to Federation 2 by adding `@link` directives ({FEDERATION_2_MIGRATION_URL}), then remove any Federation 1 pin from your configuration."
)]
pub struct FederationOneUnsupported;

/// Rover no longer supports Federation 1 at any composition-facing entry point. This is checked
/// as a standalone function so the rejection itself can be unit tested without standing up the
/// rest of `resolve_federation_version`'s subgraph-resolution machinery.
pub(crate) fn reject_federation_one(
    federation_version: &FederationVersion,
) -> Result<(), FederationOneUnsupported> {
    if federation_version.is_fed_one() {
        Err(FederationOneUnsupported)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::config::FederationVersion;
    use speculoos::prelude::*;

    use super::{FederationOneUnsupported, reject_federation_one};

    #[test]
    fn reject_federation_one_rejects_latest_fed_one() {
        let result = reject_federation_one(&FederationVersion::LatestFedOne);
        assert_that!(result).is_equal_to(Err(FederationOneUnsupported));
    }

    #[test]
    fn reject_federation_one_rejects_exact_fed_one() {
        let result =
            reject_federation_one(&FederationVersion::ExactFedOne("0.36.0".parse().unwrap()));
        assert_that!(result).is_equal_to(Err(FederationOneUnsupported));
    }

    #[test]
    fn reject_federation_one_allows_fed_two() {
        let result = reject_federation_one(&FederationVersion::LatestFedTwo);
        assert_that!(result).is_equal_to(Ok(()));
    }
}
