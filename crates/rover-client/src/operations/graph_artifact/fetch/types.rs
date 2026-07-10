use std::fmt;

use serde::Serialize;

/// The way the graph artifact to fetch is identified. Exactly one of these
/// should be provided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphArtifactIdentifier {
    /// Fetch by the artifact's content-addressable digest (SHA).
    Digest(String),
    /// Fetch by the artifact's ID.
    Id(String),
    /// Fetch the artifact currently assigned to a tag. When fetching by tag the
    /// tag's history is also returned.
    Tag(String),
}

impl fmt::Display for GraphArtifactIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Digest(digest) => write!(f, "digest '{digest}'"),
            Self::Id(id) => write!(f, "ID '{id}'"),
            Self::Tag(tag) => write!(f, "tag '{tag}'"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchGraphArtifactInput {
    pub graph_id: String,
    pub identifier: GraphArtifactIdentifier,
    /// The number of history entries to return when fetching by tag.
    pub history_limit: i64,
}

/// A single entry in a tag's history, representing one reassignment event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphArtifactHistoryEntry {
    /// The digest of the artifact the tag was assigned to in this event.
    pub digest: Option<String>,
    /// When the tag was reassigned (distinct per event, unlike the assigned
    /// artifact's own creation time).
    pub changed_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FetchGraphArtifactResponse {
    pub graph_id: String,
    pub digest: String,
    pub launch_id: String,
    pub graph_artifact_id: String,
    pub created_at: String,
    pub updated_at: String,
    /// The tag the artifact was fetched by, if any.
    pub tag: Option<String>,
    /// The tag's history, only populated when fetching by tag.
    pub history: Option<Vec<GraphArtifactHistoryEntry>>,
}
