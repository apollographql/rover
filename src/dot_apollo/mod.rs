mod project_types;
use chrono::Utc;
pub(crate) use project_types::{MultiSubgraphConfig, ProjectType, SubgraphConfig};
use saucer::{Context, Fs, Utf8PathBuf};

use serde::{Deserialize, Serialize};

use std::env;

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotApollo {
    #[serde(skip)]
    project_dir: Utf8PathBuf,
    #[serde(flatten)]
    pub(crate) project_type: ProjectType,
}

impl DotApollo {
    fn project_dir() -> Result<Utf8PathBuf> {
        Ok(env::current_dir()
            .context("could not find current directory")?
            .try_into()
            .context("current directory is not UTF-8")?)
    }

    pub(crate) fn new_subgraph(config: MultiSubgraphConfig) -> Result<Self> {
        Ok(Self {
            project_dir: Self::project_dir()?,
            project_type: ProjectType::Subgraph(config),
        })
    }

    fn get_config_dir_path() -> Result<Utf8PathBuf> {
        let config_dir_path = Self::project_dir()?.join(".apollo");
        let _ = Fs::create_dir_all(&config_dir_path, "");
        Ok(config_dir_path)
    }

    fn get_config_yaml_path() -> Result<Utf8PathBuf> {
        Ok(Self::get_config_dir_path()?
            .join("config.yaml")
            .strip_prefix(Self::project_dir().unwrap())
            .unwrap()
            .to_path_buf())
    }

    pub(crate) fn write_yaml_to_fs(&self) -> Result<()> {
        let config_path = Self::get_config_yaml_path()?;
        eprintln!("writing subgraph config to '{}'", &config_path);
        let yaml = serde_yaml::to_string(&self)?;
        let generated_on = Utc::now().format("%a %b %e, at %T %Y (utc)").to_string();
        Fs::write_file(
            &config_path,
            format!("# generated by rover on {}\n{}", generated_on, yaml),
            "",
        )?;
        Ok(())
    }

    pub(crate) fn subgraph_from_yaml() -> Result<Option<MultiSubgraphConfig>> {
        let config_path = Self::get_config_yaml_path()?;
        tracing::info!("reading subgraph config");
        if let Ok(contents) = Fs::read_file(&config_path, "") {
            let config: Self = serde_yaml::from_str(&contents)?;
            Ok(config.project_type.get_multi_subgraph())
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    fn set_project_dir(&mut self, new_dir: &Utf8PathBuf) {
        self.project_dir = new_dir.clone();
    }
}

#[cfg(test)]
mod test {
    use assert_fs::TempDir;
    use saucer::Utf8PathBuf;
    use std::convert::TryFrom;

    use super::*;

    #[test]
    fn it_can_initialize_if_no_existing_dot_apollo_dir() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
        assert!(!tmp_dir_path.join(".apollo").exists());
        let mut dot_apollo = DotApollo::new_subgraph(MultiSubgraphConfig::new()).unwrap();
        dot_apollo.set_project_dir(&tmp_dir_path);
        assert!(dot_apollo.write_yaml_to_fs().is_ok());
        assert!(tmp_dir_path.join(".apollo").exists());
    }

    #[test]
    fn it_can_initialize_if_no_existing_dot_apollo_config_yaml() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_dir_path = Utf8PathBuf::try_from(tmp_dir.path().to_path_buf()).unwrap();
        Fs::create_dir_all(tmp_dir_path.join(".apollo"), "").unwrap();
        assert!(tmp_dir_path.join(".apollo").exists());
        assert!(!tmp_dir_path.join(".apollo").join("config.yaml").exists());
        let subgraph_config = SubgraphConfig::schema()
            .file("./my-schema.sdl")
            .local_endpoint("http://localhost:4000".parse::<reqwest::Url>().unwrap())
            .build();
        let mut multi_subgraph_config = MultiSubgraphConfig::new();
        multi_subgraph_config
            .subgraph()
            .name("products")
            .config(subgraph_config)
            .add()
            .unwrap();
        let mut dot_apollo = DotApollo::new_subgraph(multi_subgraph_config).unwrap();
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
