use camino::Utf8PathBuf;

use crate::composition::CompositionError;

#[derive(thiserror::Error, Debug)]
pub enum StartCompositionError {
    #[error("Could not convert Supergraph path to URL")]
    SupergraphYamlUrlConversionFailed(Utf8PathBuf),
    #[error("Could not run initial composition")]
    InitialCompositionFailed(#[from] CompositionError),
}
