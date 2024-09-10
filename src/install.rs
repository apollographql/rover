use camino::Utf8PathBuf;

pub trait InstallBinary {
    type Binary;
    type Error;
    async fn install(&self, install_path: &Utf8PathBuf) -> Result<Self::Binary, Self::Error>;

    async fn find_existing(
        &self,
        install_path: &Utf8PathBuf,
    ) -> Result<Option<Self::Binary>, Self::Error>;

    async fn find_existing_or_install(
        &self,
        install_path: &Utf8PathBuf,
    ) -> Result<Self::Binary, Self::Error> {
        let maybe_existing = self.find_existing(install_path).await?;
        match maybe_existing {
            Some(existing) => Ok(existing),
            None => self.install(install_path).await,
        }
    }
}
