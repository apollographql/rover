use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Serialize, Clone, ValueEnum)]
pub(crate) enum PersistedQueriesManifestFormat {
    Apollo,
    Relay,
}
