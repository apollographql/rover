use async_trait::async_trait;
use camino::Utf8PathBuf;

use crate::options::LicenseAccepter;

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg_attr(test, error("MockInstallBinaryError"))]
pub struct MockInstallBinaryError {}

#[cfg(test)]
pub struct MockBinary {}

#[cfg_attr(test, mockall::automock(
    type Binary = MockBinary;
    type Error = MockInstallBinaryError;
))]
#[async_trait]
pub trait InstallBinary {
    type Binary;
    type Error: std::error::Error + Send + 'static;

    async fn install(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<Self::Binary, Self::Error>;
}
