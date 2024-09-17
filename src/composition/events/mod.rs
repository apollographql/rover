use super::{CompositionError, CompositionSuccess};

/// Events emitted from composition
pub enum CompositionEvent {
    /// The composition has started and may not have finished yet. This is useful for letting users
    /// know composition is running
    Started,
    /// Composition succeeded
    Success(CompositionSuccess),
    /// Composition errored
    Error(CompositionError),
}
