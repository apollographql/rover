#[cfg(feature = "init")]
use std::collections::HashMap;
#[cfg(feature = "init")]
use std::io::Read;

#[cfg(feature = "init")]
use crate::command::init::states::SelectedTemplateState;
#[cfg(feature = "init")]
use crate::options::{TemplateListFiles, TemplateWrite};
#[cfg(feature = "init")]
use crate::{RoverError, RoverResult};
#[cfg(feature = "init")]
use anyhow::anyhow;
#[cfg(feature = "init")]
use camino::Utf8PathBuf;
#[cfg(feature = "init")]
use rover_client::operations::init::github::{GetTarRequest, GitHubService};
#[cfg(feature = "init")]
use rover_std::Fs;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(feature = "init")]
use std::io::Cursor;
use std::str::FromStr;
#[cfg(feature = "init")]
use tower::Service;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub templates: Vec<Template>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateId(pub String);

impl FromStr for TemplateId {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(TemplateId(input.to_string()))
    }
}

impl fmt::Display for TemplateId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: TemplateId,
    pub display_name: String,
    pub path: String,
    pub language: String,
    pub federation_version: String,
    pub max_schema_depth: u32,
    pub routing_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(
        default = "Template::default_start_point_file",
        rename = "start_point_file"
    )]
    pub start_point_file: String,
}

impl Template {
    fn default_start_point_file() -> String {
        "getting-started.md".to_string()
    }
}

#[cfg(feature = "init")]
#[derive(Debug)]
pub struct InitTemplateOptions {
    pub contents: Vec<u8>,
    pub manifest: TemplateManifest,
}

#[cfg(feature = "init")]
impl InitTemplateOptions {
    pub fn list_templates(&self) -> &[Template] {
        &self.manifest.templates
    }

    pub fn select_template(&self, template_id: &TemplateId) -> RoverResult<SelectedTemplateState> {
        let template = self
            .manifest
            .templates
            .iter()
            .find(|t| t.id == *template_id)
            .ok_or_else(|| {
                RoverError::new(anyhow!("Template with id '{}' not found", template_id))
            })?;

        // Extract the tarball and store files in memory
        let cursor = Cursor::new(&self.contents);
        let tar = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(tar);

        let mut files = HashMap::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let path_str = path.to_string_lossy();

            let path_str = if let Some(stripped) = path_str.split_once('/') {
                stripped.1
            } else {
                &path_str
            };

            if path_str.starts_with(&template.path) && !entry.header().entry_type().is_dir() {
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents)?;

                let path = path_str.strip_prefix(&template.path).unwrap();
                let path = path.trim_start_matches("/");
                match Utf8PathBuf::from_str(path) {
                    Ok(path_buf) => {
                        files.insert(path_buf, contents);
                    }
                    Err(e) => {
                        return Err(RoverError::new(anyhow!(
                            "Invalid path '{}' in template '{}': {}",
                            path,
                            template.display_name,
                            e
                        )))
                    }
                }
            }
        }

        if files.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No files found in template directory '{}'",
                template.path
            )));
        }

        Ok(SelectedTemplateState {
            template: template.clone(),
            files,
        })
    }
}

#[cfg(feature = "init")]
#[derive(Debug)]
pub struct InitTemplateFetcher {
    service: GitHubService,
}

#[cfg(feature = "init")]
impl InitTemplateFetcher {
    pub fn new() -> Self {
        Self {
            service: GitHubService::new(),
        }
    }

    pub async fn call(&mut self, reference: &str) -> RoverResult<InitTemplateOptions> {
        let request = GetTarRequest::new(
            "apollographql".to_string(),
            "rover-init-starters".to_string(),
            reference.to_string(),
        );

        let contents = self.service.call(request).await?;

        if contents.is_empty() {
            return Err(RoverError::new(anyhow!("No template found")));
        }

        let cursor = Cursor::new(&contents);
        let tar = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(tar);

        let mut manifest_content = None;
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            if path.to_string_lossy().ends_with("manifest.json") {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                manifest_content = Some(content);
                break;
            }
        }

        let manifest_content = manifest_content.ok_or_else(|| {
            RoverError::new(anyhow!("manifest.json not found in template archive"))
        })?;

        let manifest: TemplateManifest = serde_json::from_str(&manifest_content)?;

        Ok(InitTemplateOptions { contents, manifest })
    }
}

#[cfg(feature = "init")]
impl TemplateListFiles for SelectedTemplateState {
    fn list_files(&self) -> RoverResult<Vec<Utf8PathBuf>> {
        Ok(self.files.keys().cloned().collect())
    }
}

#[cfg(feature = "init")]
impl TemplateWrite for SelectedTemplateState {
    fn write_template(&self, template_path: &Utf8PathBuf) -> RoverResult<()> {
        for (path, contents) in &self.files {
            let full_path = template_path.join(path);
            if let Some(parent) = full_path.parent() {
                Fs::create_dir_all(parent)?;
            }
            Fs::write_file(&full_path, contents)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_template_start_point_file_default() {
        let json = r#"{
            "id": "typescript",
            "display_name": "Build an API with Typescript",
            "path": "start-with-typescript",
            "language": "Typescript",
            "federation_version": "=2.10.0",
            "max_schema_depth": 5,
            "command": "npm ci && npm start",
            "routing_url": "http://localhost:4001"
        }"#;
        let template: Template = serde_json::from_str(json).unwrap();
        assert_eq!(template.start_point_file, "getting-started.md");
    }

    #[test]
    fn test_template_start_point_file_override() {
        let json = r#"{
            "id": "typescript",
            "display_name": "Build an API with Typescript",
            "path": "start-with-typescript",
            "language": "Typescript",
            "federation_version": "=2.10.0",
            "max_schema_depth": 5,
            "command": "npm ci && npm start",
            "routing_url": "http://localhost:4001",
            "start_point_file": "readme.md"
        }"#;
        let template: Template = serde_json::from_str(json).unwrap();
        assert_eq!(template.start_point_file, "readme.md");
    }
}
