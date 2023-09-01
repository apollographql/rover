use crate::command::dev::{router::RouterConfigState, state_machine::SubgraphKey};

/// Events handled in a `rover dev` session
pub enum Event {
    /// A subgraph schema was updated
    UpdateSubgraphSchema {
        subgraph_key: SubgraphKey,
        schema: String,
    },

    /// A subgraph schema could not be supplied
    RemoveSubgraphSchema { subgraph_key: SubgraphKey },

    /// Router configuration was updated
    UpdateRouterConfig { config: RouterConfigState },

    /// Removes router configuration
    RemoveRouterConfig,

    /// Shuts down the dev server
    Shutdown,
}
