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
