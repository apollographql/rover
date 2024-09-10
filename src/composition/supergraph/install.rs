use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;

use crate::install::InstallBinary;

use super::binary::SupergraphBinary;

#[derive(thiserror::Error, Debug)]
pub enum InstallSupergraphBinaryError {
    #[error("todo")]
    Todo,
}

pub struct InstallSupergraphBinary {
    federation_version: FederationVersion,
}

impl InstallBinary for InstallSupergraphBinary {
    type Binary = SupergraphBinary;
    type Error = InstallSupergraphBinaryError;

    async fn install(&self, install_path: &Utf8PathBuf) -> Result<Self::Binary, Self::Error> {
        todo!()
    }

    async fn find_existing(
        &self,
        install_path: &Utf8PathBuf,
    ) -> Result<Option<Self::Binary>, Self::Error> {
        todo!()
    }
}
