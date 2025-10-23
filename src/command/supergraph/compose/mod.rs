#[cfg(not(feature = "composition-js"))]
mod no_compose;

#[cfg(not(feature = "composition-js"))]
pub(crate) use no_compose::Compose;

#[cfg(feature = "composition-js")]
pub(crate) mod do_compose;

use apollo_federation_types::rover::BuildHint;
#[cfg(feature = "composition-js")]
pub(crate) use do_compose::Compose;

#[cfg(feature = "composition-js")]
use crate::composition::CompositionSuccess;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompositionOutput {
    pub supergraph_sdl: String,
    pub hints: Vec<BuildHint>,
    pub federation_version: Option<String>,
}

// Temporary conversion from new CompositionSuccess type to old CompositionOutput. In the future,
// we can change the output of run() in do_compose.rs to just be CompositionSuccess because if we
// get to the output at all, it'll be a success (we error early when things fail)
#[cfg(feature = "composition-js")]
impl From<CompositionSuccess> for CompositionOutput {
    fn from(value: CompositionSuccess) -> Self {
        Self {
            supergraph_sdl: value.supergraph_sdl().clone(),
            hints: value.hints().to_vec(),
            federation_version: Some(value.federation_version().to_string()),
        }
    }
}
