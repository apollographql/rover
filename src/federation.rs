//! All things dealing with Apollo Federation, like Composition.

mod composer;
pub(crate) mod supergraph_config;
mod watcher;

pub(crate) use composer::Composer;
pub(crate) use watcher::{Event, SubgraphSchemaWatcherKind, Watcher};
