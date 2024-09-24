//! All things dealing with Apollo Federation, like Composition.

#[cfg(feature = "composition-js")]
mod composer;
pub(crate) mod supergraph_config;
mod watcher;

#[cfg(feature = "composition-js")]
pub(crate) use composer::Composer;
#[cfg(feature = "composition-js")]
pub(crate) use watcher::{Event, SubgraphSchemaWatcherKind, Watcher};
