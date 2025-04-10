use crate::command::init::config::ProjectConfig;
use crate::options::{ProjectName, ProjectType, ProjectUseCase, TemplateProject};
use camino::Utf8PathBuf;

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
    pub organization: String,
}

#[derive(Debug)]
pub struct UseCaseSelected {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
}

#[derive(Debug)]
pub struct ProjectNamed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
}

#[derive(Debug)]
pub struct GraphIdConfirmed {
    pub output_path: Utf8PathBuf,
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: String,
}

#[derive(Debug)]
pub struct CreationConfirmed {
    pub output_path: Utf8PathBuf,
    pub config: ProjectConfig,
    pub template: TemplateProject,
}

#[derive(Debug)]
pub struct ProjectCreated {
    pub config: ProjectConfig,
    pub artifacts: Vec<Utf8PathBuf>,
    pub api_key: String,
}

#[derive(Debug)]
pub struct Completed;
