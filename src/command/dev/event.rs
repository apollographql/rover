/// Events handled in a `rover dev` session
pub(crate) enum Event {
    /// A subgraph schema was updated
    UpdateSubgraphSchema { name: String, schema: String },

    /// A subgraph schema could not be supplied
    NoSubgraphSchema { name: String },
}
