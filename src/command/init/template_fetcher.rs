use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::{Cursor, Read},
    str::FromStr,
};

use anyhow::anyhow;
use camino::Utf8PathBuf;
use rover_client::operations::init::github::{GetTarRequest, GitHubService};
use rover_std::Fs;
use serde::{Deserialize, Serialize};
use tower::Service;

use crate::{
    RoverError, RoverResult,
    command::init::states::SelectedTemplateState,
    options::{TemplateListFiles, TemplateWrite},
};

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
    pub commands: Option<Vec<String>>,
    #[serde(
        default = "Template::default_start_point_file",
        rename = "start_point_file"
    )]
    pub start_point_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_depth: Option<u8>,
}

impl Template {
    fn default_start_point_file() -> String {
        "GETTING_STARTED.md".to_string()
    }
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
                        )));
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

    /// Extract files directly from a directory in the repository (not a template)
    pub fn extract_directory_files(
        &self,
        directory_path: &str,
    ) -> RoverResult<HashMap<Utf8PathBuf, String>> {
        let cursor = Cursor::new(&self.contents);
        let tar = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(tar);

        let mut files = HashMap::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let path_str = path.to_string_lossy();

            // Strip the repo name prefix (e.g., "rover-init-starters-camille-start-with-mcp-template/")
            let path_str = if let Some(stripped) = path_str.split_once('/') {
                stripped.1
            } else {
                &path_str
            };

            // Only include files from the specified directory
            if let Some(relative_path) = path_str.strip_prefix(&format!("{}/", directory_path)) {
                // Skip directories themselves
                if entry.header().entry_type().is_file() {
                    let mut content = String::new();
                    entry.read_to_string(&mut content)?;

                    // Convert to Utf8PathBuf
                    let utf8_path = Utf8PathBuf::from(relative_path);
                    files.insert(utf8_path, content);
                }
            }
        }

        if files.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No files found in directory '{}'",
                directory_path
            )));
        }

        Ok(files)
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

    /// Fetch and compose MCP template (base template + add-mcp)
    pub async fn fetch_mcp_template(
        &mut self,
        base_template_id: &str,
        reference: &str,
    ) -> RoverResult<SelectedTemplateState> {
        // First, get the template manifest
        let template_options = self.call(reference).await?;

        // Find the base template
        let base_template = template_options
            .manifest
            .templates
            .iter()
            .find(|t| t.id.0 == base_template_id)
            .ok_or_else(|| {
                RoverError::new(anyhow!("Base template '{}' not found", base_template_id))
            })?;

        // Extract base template files
        let mut base_state = Self::extract_template_files(&template_options, base_template)?;

        // Fetch add-mcp template and merge it
        let add_mcp_template = Template {
            id: TemplateId("add-mcp".to_string()),
            display_name: "MCP Augmentation".to_string(),
            path: "add-mcp".to_string(),
            language: "".to_string(),
            federation_version: "".to_string(),
            max_schema_depth: 0,
            routing_url: "".to_string(),
            commands: None,
            start_point_file: "".to_string(),
            print_depth: None,
        };

        let mcp_state = Self::extract_template_files(&template_options, &add_mcp_template)?;

        // Merge MCP files into base template with intelligent merging for special files
        for (mcp_path, mcp_contents) in mcp_state.files {
            if mcp_path.as_str() == ".gitignore" {
                // Special handling for .gitignore files - merge instead of overwrite
                if let Some(base_gitignore) = base_state.files.get(&mcp_path) {
                    let merged_gitignore =
                        Self::merge_gitignore_files(base_gitignore, &mcp_contents)?;
                    base_state.files.insert(mcp_path, merged_gitignore);
                } else {
                    base_state.files.insert(mcp_path, mcp_contents);
                }
            } else {
                base_state.files.insert(mcp_path, mcp_contents);
            }
        }

        // Create MCP template metadata
        let mcp_template = Template {
            id: TemplateId(format!("mcp-{}", base_template_id)),
            display_name: format!("{} + AI tools", base_template.display_name),
            path: base_template.path.clone(),
            language: base_template.language.clone(),
            federation_version: base_template.federation_version.clone(),
            max_schema_depth: base_template.max_schema_depth,
            routing_url: base_template.routing_url.clone(),
            commands: base_template.commands.clone(),
            start_point_file: base_template.start_point_file.clone(),
            print_depth: base_template.print_depth,
        };

        base_state.template = mcp_template;
        Ok(base_state)
    }

    /// Helper method to extract files from template
    fn extract_template_files(
        template_options: &InitTemplateOptions,
        template: &Template,
    ) -> RoverResult<SelectedTemplateState> {
        template_options.select_template(&template.id)
    }

    /// Merge two .gitignore files intelligently, preserving unique patterns
    fn merge_gitignore_files(base_content: &[u8], mcp_content: &[u8]) -> RoverResult<Vec<u8>> {
        let base_str = String::from_utf8_lossy(base_content);
        let mcp_str = String::from_utf8_lossy(mcp_content);

        // Track unique patterns to avoid duplicates
        let mut patterns = HashSet::new();
        let mut result = Vec::new();

        // Add base template section header and patterns
        result.push("# Base template ignores".to_string());
        for line in base_str.lines() {
            let trimmed = line.trim();
            // Preserve comments and empty lines as-is
            if trimmed.is_empty() || trimmed.starts_with('#') {
                result.push(line.to_string());
            } else {
                // Only add unique patterns
                if patterns.insert(trimmed.to_string()) {
                    result.push(line.to_string());
                }
            }
        }

        // Add separator and MCP section
        result.push("".to_string()); // empty line
        result.push("# MCP server additions".to_string());
        for line in mcp_str.lines() {
            let trimmed = line.trim();
            // Preserve comments and empty lines as-is
            if trimmed.is_empty() || trimmed.starts_with('#') {
                result.push(line.to_string());
            } else {
                // Only add unique patterns
                if patterns.insert(trimmed.to_string()) {
                    result.push(line.to_string());
                }
            }
        }

        // Convert back to bytes
        let merged_content = result.join("\n");
        Ok(merged_content.into_bytes())
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

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn test_template_start_point_file_default() {
        let json = r#"{
            "id": "typescript",
            "display_name": "Build an API with TypeScript",
            "path": "start-with-typescript",
            "language": "Typescript",
            "federation_version": "=2.10.0",
            "max_schema_depth": 5,
            "commands": ["npm ci", "npm start"],
            "routing_url": "http://localhost:4001"
        }"#;
        let template: Template = serde_json::from_str(json).unwrap();
        assert_eq!(template.start_point_file, "GETTING_STARTED.md");
    }

    #[test]
    fn test_template_start_point_file_override() {
        let json = r#"{
            "id": "typescript",
            "display_name": "Build an API with TypeScript",
            "path": "start-with-typescript",
            "language": "Typescript",
            "federation_version": "=2.10.0",
            "max_schema_depth": 5,
            "commands": ["npm ci", "npm start"],
            "routing_url": "http://localhost:4001",
            "start_point_file": "readme.md"
        }"#;
        let template: Template = serde_json::from_str(json).unwrap();
        assert_eq!(template.start_point_file, "readme.md");
    }

    #[test]
    fn test_merge_gitignore_files() {
        let base_gitignore = b"# TypeScript ignores\nnode_modules/\n*.log\ndist/\n";
        let mcp_gitignore = b"# MCP ignores\n.env\n*.log\ntools/temp/\n";

        let result = InitTemplateFetcher::merge_gitignore_files(base_gitignore, mcp_gitignore)
            .expect("Failed to merge gitignore files");

        let merged_str = String::from_utf8(result).expect("Invalid UTF-8");

        // Should contain both sets of patterns
        assert!(merged_str.contains("node_modules/"));
        assert!(merged_str.contains(".env"));
        assert!(merged_str.contains("dist/"));
        assert!(merged_str.contains("tools/temp/"));

        // Should have section headers
        assert!(merged_str.contains("# Base template ignores"));
        assert!(merged_str.contains("# MCP server additions"));

        // Should not duplicate *.log pattern
        let log_count = merged_str.matches("*.log").count();
        assert_eq!(log_count, 1, "*.log should only appear once in merged file");
    }

    #[test]
    fn test_merge_gitignore_files_preserves_comments() {
        let base_gitignore =
            b"# Important comment\n# Another comment\nnode_modules/\n\n# Section\ndist/\n";
        let mcp_gitignore = b"# MCP comment\n.env\n";

        let result = InitTemplateFetcher::merge_gitignore_files(base_gitignore, mcp_gitignore)
            .expect("Failed to merge gitignore files");

        let merged_str = String::from_utf8(result).expect("Invalid UTF-8");

        // Should preserve original comments
        assert!(merged_str.contains("# Important comment"));
        assert!(merged_str.contains("# Another comment"));
        assert!(merged_str.contains("# MCP comment"));
        assert!(merged_str.contains("# Section"));

        // Should preserve empty lines structure
        assert!(merged_str.contains("\n\n"));
    }

    #[test]
    fn test_merge_gitignore_files_no_base_file() {
        let base_gitignore = b"";
        let mcp_gitignore = b"# MCP ignores\n.env\ntools/temp/\n";

        let result = InitTemplateFetcher::merge_gitignore_files(base_gitignore, mcp_gitignore)
            .expect("Failed to merge gitignore files");

        let merged_str = String::from_utf8(result).expect("Invalid UTF-8");

        // Should contain MCP patterns
        assert!(merged_str.contains(".env"));
        assert!(merged_str.contains("tools/temp/"));

        // Should still have proper headers
        assert!(merged_str.contains("# Base template ignores"));
        assert!(merged_str.contains("# MCP server additions"));
    }
}
