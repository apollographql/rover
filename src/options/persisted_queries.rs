use clap::ValueEnum;

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum PersistedQueriesManifestFormat {
    Apollo,
    Relay,
}
