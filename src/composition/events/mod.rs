use super::{
    CompositionError, CompositionSubgraphAdded, CompositionSubgraphRemoved, CompositionSuccess,
};

/// Events emitted from composition
#[derive(Debug)]
pub enum CompositionEvent {
    /// The composition has started and may not have finished yet. This is useful for letting users
    /// know composition is running
    Started,
    /// Composition succeeded
    Success(CompositionSuccess),
    /// Composition errored
    Error(CompositionError),
    /// Subgraph Added
    SubgraphAdded(CompositionSubgraphAdded),
    /// SubgraphRemoved
    SubgraphRemoved(CompositionSubgraphRemoved),
}
