use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListTagsInput {
    ByGraph { graph_id: String },
    ByDigest { graph_id: String, digest: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ListTagEntry {
    pub tag: String,
    pub digest: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ListTagsResponse {
    pub tags: Vec<ListTagEntry>,
}
