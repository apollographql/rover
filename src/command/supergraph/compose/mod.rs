#[cfg(not(feature = "composition-js"))]
mod no_compose;

#[cfg(not(feature = "composition-js"))]
pub(crate) use no_compose::Compose;

#[cfg(feature = "composition-js")]
mod do_compose;

#[cfg(feature = "composition-js")]
pub(crate) use do_compose::Compose;

use apollo_federation_types::rover::BuildHint;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompositionOutput {
    pub supergraph_sdl: String,
    pub hints: Vec<BuildHint>,
    pub federation_version: Option<String>,
}
