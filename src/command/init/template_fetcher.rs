use std::collections::HashMap;
use std::io::Read;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::str::FromStr;
use tower::Service;

use crate::command::init::states::SelectedTemplateState;
use crate::options::{TemplateListFiles, TemplateWrite};
use crate::{RoverError, RoverResult};
use rover_client::operations::init::github::{GetTarRequest, GitHubService};
use rover_std::Fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub templates: Vec<Template>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub language: String,
    pub federation_version: String,
    pub max_schema_depth: u32,
    pub routing_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug)]
pub struct InitTemplateOptions {
    pub contents: Vec<u8>,
    pub manifest: TemplateManifest,
}

impl InitTemplateOptions {
    pub fn list_templates(&self) -> &[Template] {
        &self.manifest.templates
    }

    pub fn select_template(&self, template_id: &str) -> RoverResult<SelectedTemplateState> {
        let template = self
            .manifest
            .templates
            .iter()
            .find(|t| t.id == template_id)
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

#[derive(Debug)]
pub struct InitTemplateFetcher {
    service: GitHubService,
}

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

impl TemplateListFiles for SelectedTemplateState {
    fn list_files(&self) -> RoverResult<Vec<Utf8PathBuf>> {
        Ok(self.files.keys().cloned().collect())
    }
}

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
