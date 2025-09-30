use crate::command::init::config::ProjectConfig;
use crate::command::init::graph_id::validation::GraphId;
use crate::command::init::options::{OrganizationId, ProjectName, ProjectType, ProjectUseCase};
use crate::command::init::template_fetcher::Template;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use std::collections::HashMap;

#[derive(Debug)]
pub struct UserAuthenticated;

#[derive(Debug)]
pub struct Welcome {}

#[derive(Debug)]
pub struct ProjectTypeSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
}

#[derive(Debug)]
pub struct OrganizationSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
}

#[derive(Debug)]
pub struct UseCaseSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
}

pub struct TemplateSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub selected_template: SelectedTemplateState,
}

#[derive(Debug)]
pub struct ProjectNamed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub selected_template: SelectedTemplateState,
    pub project_name: ProjectName,
}

#[derive(Debug)]
pub struct GraphIdConfirmed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: GraphId,
    pub selected_template: SelectedTemplateState,
}

#[derive(Debug)]
pub struct CreationConfirmed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub selected_template: SelectedTemplateState,
}

#[derive(Debug)]
pub struct ProjectCreated {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub artifacts: Vec<Utf8PathBuf>,
    pub api_key: String,
    pub graph_ref: GraphRef,
    pub template: Template,
}

#[derive(Debug)]
pub struct Completed;

#[derive(Debug)]
pub struct SelectedTemplateState {
    pub template: Template,
    pub files: HashMap<Utf8PathBuf, Vec<u8>>,
}

// MCP-specific enums and states for handling MCP augmentation flow
// These states branch from the main init flow and have different behavior

#[derive(Clone, Debug)]
pub enum MCPSetupType {
    ExistingGraph,
    NewProject,
}

impl std::fmt::Display for MCPSetupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPSetupType::ExistingGraph => {
                write!(
                    f,
                    "Create MCP tools from an existing Apollo GraphOS project"
                )
            }
            MCPSetupType::NewProject => {
                write!(f, "Create MCP tools from a new Apollo GraphOS project")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum MCPDataSourceType {
    ExternalAPIs, // REST, webhooks, SaaS
    GraphQLAPI,   // Existing GraphQL endpoints
}

impl std::fmt::Display for MCPDataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPDataSourceType::ExternalAPIs => {
                write!(f, "Apollo graph with Connectors (connect to REST services)")
            }
            MCPDataSourceType::GraphQLAPI => write!(
                f,
                "Apollo graph with GraphQL endpoints (connect to existing GraphQL endpoints)"
            ),
        }
    }
}

#[derive(Debug)]
pub struct MCPInitialization {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
}

#[derive(Debug)]
pub struct MCPSetupTypeSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub setup_type: MCPSetupType,
}

#[derive(Debug)]
pub struct MCPDataSourceSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub setup_type: MCPSetupType,
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPOrganizationSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub setup_type: MCPSetupType,
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPTemplateComposed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub composed_template: MCPComposedTemplate,
    pub setup_type: MCPSetupType,
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPProjectNamed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub composed_template: MCPComposedTemplate,
    pub project_name: ProjectName,
    pub setup_type: MCPSetupType,
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPGraphIdConfirmed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: GraphId,
    pub composed_template: MCPComposedTemplate,
    pub setup_type: MCPSetupType,
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPCreationPreviewed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub composed_template: MCPComposedTemplate,
    #[allow(dead_code)] // Used when composition-js feature is enabled
    pub setup_type: MCPSetupType,
    #[allow(dead_code)] // Used when composition-js feature is enabled
    pub data_source_type: MCPDataSourceType,
}

#[derive(Debug)]
pub struct MCPCreationConfirmed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub composed_template: MCPComposedTemplate,
}

/// Represents an MCP template that's been composed from a base template + MCP additions
#[derive(Debug)]
pub struct MCPComposedTemplate {
    pub base_template: Template,
    pub merged_files: HashMap<Utf8PathBuf, Vec<u8>>,
    #[allow(dead_code)]
    pub agents_merged_into_readme: bool,
}

impl MCPComposedTemplate {
    #[allow(dead_code)]
    pub fn new(base_template: Template, merged_files: HashMap<Utf8PathBuf, Vec<u8>>) -> Self {
        // The merged_files should already contain base template + MCP additions
        // as they are pre-merged by template_fetcher.rs
        Self {
            base_template,
            merged_files,
            agents_merged_into_readme: false,
        }
    }

    /// Create a new MCPComposedTemplate for new projects with AGENTS.md merged into README.md
    pub fn new_with_agents_merge(
        base_template: Template,
        mut merged_files: HashMap<Utf8PathBuf, Vec<u8>>,
        project_type: crate::command::init::options::ProjectType,
    ) -> Self {
        use crate::command::init::options::ProjectType;

        // Check if AGENTS.md exists before merging
        let agents_path = camino::Utf8PathBuf::from("AGENTS.md");
        let has_agents_md = merged_files.contains_key(&agents_path);

        // Only merge for NEW projects
        let agents_merged = if project_type == ProjectType::CreateNew && has_agents_md {
            merged_files = Self::merge_agents_into_readme(merged_files);
            true
        } else {
            false
        };

        Self {
            base_template,
            merged_files,
            agents_merged_into_readme: agents_merged,
        }
    }

    /// Merge AGENTS.md content into README.md while keeping AGENTS.md as a separate file
    fn merge_agents_into_readme(
        mut files: HashMap<Utf8PathBuf, Vec<u8>>,
    ) -> HashMap<Utf8PathBuf, Vec<u8>> {
        use camino::Utf8PathBuf;

        let agents_path = Utf8PathBuf::from("AGENTS.md");
        let readme_path = Utf8PathBuf::from("README.md");

        // Get AGENTS.md content if it exists (without removing it)
        let agents_content = match files.get(&agents_path) {
            Some(content) => content.clone(),
            None => return files, // No AGENTS.md, nothing to merge
        };

        // Get or create README.md
        let readme_content = files.remove(&readme_path).unwrap_or_else(|| Vec::new());

        // Convert to strings for processing
        let mut readme_str = String::from_utf8_lossy(&readme_content).to_string();
        let agents_str = String::from_utf8_lossy(&agents_content);

        // Ensure README ends with newlines before appending
        if !readme_str.is_empty() && !readme_str.ends_with("\n\n") {
            if !readme_str.ends_with('\n') {
                readme_str.push('\n');
            }
            readme_str.push('\n');
        }

        // Append AGENTS.md content to README.md
        readme_str.push_str(&agents_str);

        // Put the updated README.md back
        files.insert(readme_path, readme_str.into_bytes());

        files
    }

    pub fn list_files(&self) -> Vec<Utf8PathBuf> {
        self.merged_files.keys().cloned().collect()
    }
}
