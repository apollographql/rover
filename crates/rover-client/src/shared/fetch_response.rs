use serde::Serialize;

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct FetchResponse {
    pub sdl: Sdl,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct Sdl {
    pub contents: String,
    #[serde(skip_serializing)]
    pub r#type: SdlType,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
#[serde(rename_all(serialize = "lowercase"))]
pub enum SdlType {
    Graph,
    Subgraph { routing_url: Option<String> },
    Supergraph,
}
