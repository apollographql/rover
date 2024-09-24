//! All things dealing with Apollo Federation, like Composition.

#[cfg(feature = "composition-js")]
mod composer;
pub(crate) mod supergraph_config;
mod watcher;

use apollo_federation_types::config::FederationVersion;
#[cfg(feature = "composition-js")]
pub(crate) use composer::Composer;
#[cfg(feature = "composition-js")]
pub(crate) use watcher::{Event, SubgraphSchemaWatcherKind, Watcher};

/// Format a [`FederationVersion`] (coming from an exact version, which includes a `=` rather than a
/// `v`) for readability
pub(crate) fn format_version(version: &FederationVersion) -> String {
    let unformatted = &version.to_string()[1..];
    let mut formatted = unformatted.to_string();
    formatted.insert(0, 'v');
    formatted
}
