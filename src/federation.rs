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
/// (composition, `rover install --plugin`, `rover init`), so the rejection message stays
/// consistent.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "Federation 1 is no longer supported by Rover. Migrate your subgraphs to Federation 2 by adding `@link` directives ({FEDERATION_2_MIGRATION_URL}), then remove any Federation 1 pin from your configuration."
)]
pub struct FederationOneUnsupported;

/// Whether `input` is an exact version pin (`=0.36.0`, `v1.0.0`) below Federation 2. Such a pin
/// is Federation 1 by any reading, but [`FederationVersion`]'s own parser doesn't accept every
/// one of them (`=1.0.0` isn't a supergraph plugin release at all), so it can't be left to
/// [`reject_federation_one`] to catch it after parsing.
pub(crate) fn is_federation_one_exact_pin(input: &str) -> bool {
    input
        .strip_prefix(['=', 'v'])
        .and_then(|version| semver::Version::parse(version).ok())
        .is_some_and(|version| version.major < 2)
}

/// Parses a `--federation-version` value: [`FederationVersion`]'s own grammar, except that a
/// Federation 1 exact pin is refused with the Federation 1 message however it is spelled, and
/// a value that isn't a version at all is not told that `1` would do, since it is refused.
pub(crate) fn parse_federation_version(input: &str) -> Result<FederationVersion, String> {
    match input.parse::<FederationVersion>() {
        Ok(version) => Ok(version),
        Err(_) if is_federation_one_exact_pin(input) => Err(FederationOneUnsupported.to_string()),
        Err(_) => Err(format!(
            "Specified version `{input}` is not supported. You can specify '2', or a fully qualified version prefixed with an '=', like: =2.0.0"
        )),
    }
}

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

/// Whether `error` is, or wraps, a [`FederationOneUnsupported`] refusal. The composition and
/// `init` errors that carry one do so `#[error(transparent)]`, which hides it from the error
/// chain, so each is checked for by name.
pub(crate) fn is_federation_one_unsupported(error: &anyhow::Error) -> bool {
    use crate::{
        command::init::operations::GraphOperationError,
        composition::{CompositionError, pipeline::CompositionPipelineError},
    };

    error.downcast_ref::<FederationOneUnsupported>().is_some()
        || matches!(
            error.downcast_ref::<CompositionError>(),
            Some(CompositionError::FederationOneUnsupported(_))
        )
        || matches!(
            error.downcast_ref::<CompositionPipelineError>(),
            Some(CompositionPipelineError::FederationOneUnsupported(_))
        )
        || matches!(
            error.downcast_ref::<GraphOperationError>(),
            Some(GraphOperationError::FederationOneUnsupported(_))
        )
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::config::FederationVersion;
    use speculoos::prelude::*;

    use super::{FederationOneUnsupported, reject_federation_one};
    use crate::{
        RoverError, RoverErrorCode,
        command::init::operations::GraphOperationError,
        composition::{CompositionError, pipeline::CompositionPipelineError},
    };

    /// Every way the refusal reaches the user carries E064, so a script can match on
    /// `error.code`, including through the `#[error(transparent)]` wrappers that hide it from
    /// the error chain.
    #[rstest::rstest]
    #[case::bare(anyhow::Error::new(FederationOneUnsupported))]
    #[case::composition(anyhow::Error::new(CompositionError::FederationOneUnsupported(
        FederationOneUnsupported
    )))]
    #[case::pipeline(anyhow::Error::new(CompositionPipelineError::FederationOneUnsupported(
        FederationOneUnsupported
    )))]
    #[case::init(anyhow::Error::new(GraphOperationError::FederationOneUnsupported(
        FederationOneUnsupported
    )))]
    fn the_refusal_has_its_own_error_code(#[case] error: anyhow::Error) {
        assert_that!(RoverError::new(error).code()).is_equal_to(Some(RoverErrorCode::E064));
    }

    #[test]
    fn an_unrelated_composition_error_has_no_such_code() {
        let error =
            anyhow::Error::new(CompositionError::InvalidSupergraphConfig("bad".to_string()));
        assert_that!(RoverError::new(error).code()).is_none();
    }

    #[rstest::rstest]
    #[case::exact_one("=1.0.0", true)]
    #[case::exact_zero("=0.36.0", true)]
    #[case::v_prefixed("v1.2.3", true)]
    #[case::exact_two("=2.9.0", false)]
    #[case::exact_three("=3.0.0", false)]
    #[case::bare_one("1", false)]
    #[case::not_a_version("banana", false)]
    fn only_an_exact_pin_below_federation_two_counts(#[case] input: &str, #[case] expected: bool) {
        assert_that!(super::is_federation_one_exact_pin(input)).is_equal_to(expected);
    }

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
