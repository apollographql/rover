#[derive(Debug, Clone, PartialEq)]
pub struct FetchResponse {
    pub sdl: Sdl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sdl {
    pub contents: String,
    pub r#type: SdlType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SdlType {
    Graph,
    Subgraph,
    Supergraph,
}
