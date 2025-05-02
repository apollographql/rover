use crate::command::init::config::ProjectConfig;
use crate::command::init::graph_id::validation::GraphId;
use crate::command::init::options::{OrganizationId, ProjectName, ProjectType, ProjectUseCase};
#[cfg(feature = "init")]
use crate::command::init::Template;
#[cfg(not(feature = "init"))]
use crate::options::TemplateProject;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
#[cfg(feature = "init")]
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

#[cfg(feature = "init")]
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
    #[cfg(feature = "init")]
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
    #[cfg(feature = "init")]
    pub selected_template: SelectedTemplateState,
}

#[derive(Debug)]
#[cfg(feature = "init")]
pub struct CreationConfirmed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub selected_template: SelectedTemplateState,
}

#[derive(Debug)]
#[cfg(not(feature = "init"))]
pub struct CreationConfirmed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub selected_template: TemplateProject,
}

#[derive(Debug)]
pub struct ProjectCreated {
    pub config: ProjectConfig,
    pub artifacts: Vec<Utf8PathBuf>,
    pub api_key: String,
    pub graph_ref: GraphRef,
}

#[derive(Debug)]
pub struct Completed;

#[derive(Debug)]
#[cfg(feature = "init")]
pub struct SelectedTemplateState {
    pub template: Template,
    pub files: HashMap<Utf8PathBuf, Vec<u8>>,
}
