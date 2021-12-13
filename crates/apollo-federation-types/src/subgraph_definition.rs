use serde::Serialize;

/// The `SubgraphDefinition` represents everything we need to know about a
/// subgraph for its GraphQL runtime responsibilities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubgraphDefinition {
    /// The name of the subgraph. We use this name internally to
    /// in the representation of the composed schema and for designations
    /// within the human-readable QueryPlan.
    pub name: String,

    /// The routing/runtime URL where the subgraph can be found that will
    /// be able to fulfill the requests it is responsible for.
    pub url: String,

    /// The Schema Definition Language (SDL) containing the type definitions
    /// for a subgraph.
    pub sdl: String,
}

impl SubgraphDefinition {
    /// Create a new SubgraphDefinition
    pub fn new<N: Into<String>, U: Into<String>, S: Into<String>>(
        name: N,
        url: U,
        sdl: S,
    ) -> SubgraphDefinition {
        SubgraphDefinition {
            name: name.into(),
            url: url.into(),
            sdl: sdl.into(),
        }
    }
}
