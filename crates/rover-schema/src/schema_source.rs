use rover_studio::types::GraphRef;
use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;

#[derive(thiserror::Error, Debug)]
#[error("Invalid schema source")]
pub struct InvalidSchemaSource;

#[serde_as]
#[derive(Debug, Clone, Serialize)]
pub enum SchemaSource {
    GraphOS(GraphRef),
    File(PathBuf),
    Url(#[serde_as(as = "DisplayFromStr")] Url),
}

impl fmt::Display for SchemaSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaSource::GraphOS(graph_ref) => write!(f, "{}", graph_ref),
            SchemaSource::File(path) => write!(f, "{}", path.display()),
            SchemaSource::Url(url) => write!(f, "{}", url),
        }
    }
}

impl FromStr for SchemaSource {
    type Err = InvalidSchemaSource;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GraphRef::from_str(s)
            .map(SchemaSource::GraphOS)
            .map_err(|_| InvalidSchemaSource)
            .or_else(|_| {
                Url::parse(s)
                    .map_err(|_| InvalidSchemaSource)
                    .and_then(|url| {
                        if matches!(url.scheme(), "http" | "https") {
                            Ok(SchemaSource::Url(url))
                        } else {
                            Err(InvalidSchemaSource)
                        }
                    })
            })
            .or_else(|_| {
                let path = Path::new(s);
                if path.exists() {
                    Ok(SchemaSource::File(path.to_path_buf()))
                } else {
                    Err(InvalidSchemaSource)
                }
            })
    }
}
