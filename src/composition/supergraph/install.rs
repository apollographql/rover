use std::{fs, str::FromStr};

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use semver::Version;

use crate::install::InstallBinary;

use super::{binary::SupergraphBinary, version::SupergraphVersion};

#[derive(thiserror::Error, Debug)]
pub enum InstallSupergraphBinaryError {
    #[error("Invalid install path: `{0}`")]
    InvalidInstallPath(Utf8PathBuf),
}

pub struct InstallSupergraphBinary {
    federation_version: FederationVersion,
}

impl InstallBinary for InstallSupergraphBinary {
    type Binary = SupergraphBinary;
    type Error = InstallSupergraphBinaryError;

    async fn install(&self, install_path: &Utf8PathBuf) -> Result<Self::Binary, Self::Error> {
        // Attempt to create the install path if it doesn't already exist.
        fs::create_dir_all(install_path)
            .map_err(|_| Self::Error::InvalidInstallPath(install_path.clone()))?;

        // TODO: download bin and write to install_path/exe.

        Ok(Self::Binary::new(
            // TODO: need to construct path here.
            // Where are we actually getting the exe name from?
            install_path.clone(),
            SupergraphVersion::new(Version::from_str("blah").unwrap()),
        ))
    }

    async fn find_existing(
        &self,
        install_path: &Utf8PathBuf,
    ) -> Result<Option<Self::Binary>, Self::Error> {
        // TODO: join exe to path.
        let bin: Utf8PathBuf = install_path.join("");

        // Nothing exists at the given install_path,
        // so we can assume a supergraph binary is not installed.
        if !bin.exists() {
            return Ok(None);
        }

        Ok(Some(Self::Binary::new(
            // TODO: need to construct path here.
            // Where are we actually getting the exe name from?
            bin,
            SupergraphVersion::new(Version::from_str("blah").unwrap()),
        )))
    }
}
