use crate::command::dev::protocol::SubgraphKey;

/// Events handled in a `rover dev` session
pub(crate) enum Event {
    /// A subgraph schema was updated
    UpdateSubgraphSchema {
        subgraph_key: SubgraphKey,
        schema: String,
    },

    /// A subgraph schema could not be supplied
    RemoveSubgraphSchema { subgraph_key: SubgraphKey },

    /// Router configuration was updated
    UpdateRouterConfig { config: String },

    /// Removes router configuration
    RemoveRouterConfig,
}
