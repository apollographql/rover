mod project_types;
use camino::Utf8PathBuf;
pub(crate) use project_types::{ProjectType, SubgraphProjectConfig};

use serde::{Deserialize, Serialize};

use std::{env, fs};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotApollo {
    #[serde(skip_serializing)]
    project_dir: Utf8PathBuf,
    #[serde(flatten)]
    pub(crate) project_type: ProjectType,
}

impl DotApollo {
    fn project_dir() -> Result<Utf8PathBuf> {
        Ok(env::current_dir()?.try_into()?)
    }

    pub(crate) fn new_subgraph(config: SubgraphProjectConfig) -> Result<Self> {
        Ok(Self {
            project_dir: Self::project_dir()?,
            project_type: ProjectType::Subgraph(config),
        })
    }

    fn get_config_dir_path(&self) -> Utf8PathBuf {
        let config_dir_path = self.project_dir.join(".apollo");
        let _ = fs::create_dir_all(&config_dir_path);
        config_dir_path
    }

    fn get_config_yaml_path(&self) -> Utf8PathBuf {
        self.get_config_dir_path().join("config.yaml")
    }

    pub(crate) fn write_yaml_to_fs(&self) -> Result<()> {
        let config_path = self.get_config_yaml_path();
        tracing::debug!("writing config to {}", &config_path);
        fs::write(&config_path, serde_yaml::to_string(&self)?)?;
        Ok(())
    }

    pub(crate) fn read_yaml_from_fs(&self) -> Result<Self> {
        let config_path = self.get_config_yaml_path();
        tracing::debug!("reading config from {}", &config_path);
        let raw_contents = fs::read_to_string(&config_path)?;
        let config: Self = serde_yaml::from_str(&raw_contents)?;
        Ok(config)
    }

    #[cfg(test)]
    fn set_project_dir(&mut self, new_dir: &Utf8PathBuf) {
        self.project_dir = new_dir.clone();
    }
}

#[cfg(test)]
mod test {
    use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use std::convert::TryFrom;

    use super::*;

    #[test]
    fn it_can_initialize_if_no_existing_dot_apollo_dir() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
        assert!(!tmp_dir_path.join(".apollo").exists());
        let mut dot_apollo = DotApollo::new_subgraph(SubgraphProjectConfig::new(
            Some("my-supergraph".to_string()),
            SubgraphConfig {
                routing_url: Some("https://my-routing.url.com".to_string()),
                schema: SchemaSource::File {
                    file: "./my-schema.sdl".into(),
                },
            },
        ))
        .unwrap();
        dot_apollo.set_project_dir(&tmp_dir_path);
        assert!(dot_apollo.write_yaml_to_fs().is_ok());
        assert!(tmp_dir_path.join(".apollo").exists());
    }

    #[test]
    fn it_can_initialize_if_no_existing_dot_apollo_config_yaml() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
        fs::create_dir_all(tmp_dir_path.join(".apollo")).unwrap();
        assert!(tmp_dir_path.join(".apollo").exists());
        assert!(!tmp_dir_path.join(".apollo").join("config.yaml").exists());
        let mut dot_apollo = DotApollo::new_subgraph(SubgraphProjectConfig::new(
            Some("my-supergraph".to_string()),
            SubgraphConfig {
                routing_url: Some("https://my-routing.url.com".to_string()),
                schema: SchemaSource::File {
                    file: "./my-schema.sdl".into(),
                },
            },
        ))
        .unwrap();
        dot_apollo.set_project_dir(&tmp_dir_path);
        assert!(dot_apollo.write_yaml_to_fs().is_ok());
        assert!(tmp_dir_path.join(".apollo").join("config.yaml").exists());
    }

    // #[test]
    // fn it_can_initialize_a_new_variant_in_dot_apollo_config_yaml() {
    //     let tmp_dir = TempDir::new().unwrap();
    //     let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
    //     let subgraph = DotApollo::new_subgraph(&tmp_dir_path);
    //     assert!(subgraph.init(&tmp_dir_path).is_ok());
    // }

    // #[test]
    // fn it_cannot_initialize_a_preexisting_variant_in_dot_apollo_config_yaml() {
    //     let tmp_dir = TempDir::new().unwrap();
    //     let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
    //     fs::create_dir_all(tmp_dir_path.join(".apollo"));
    //     assert!(dot_apollo.init(&tmp_dir_path).is_err());
    // }
}
