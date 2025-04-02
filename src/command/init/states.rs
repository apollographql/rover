use crate::command::init::config::ProjectConfig;
use crate::options::{ProjectType, ProjectUseCase, TemplateProject};
use camino::Utf8PathBuf;

#[derive(Debug)]
pub struct Welcome;

#[derive(Debug)]
pub struct ProjectTypeSelected {
    pub project_type: ProjectType,
}

#[derive(Debug)]
pub struct OrganizationSelected {
    pub project_type: ProjectType,
    pub organization: String,
}

#[derive(Debug)]
pub struct UseCaseSelected {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
}

#[derive(Debug)]
pub struct ProjectNamed {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: String,
}

#[derive(Debug)]
pub struct GraphIdConfirmed {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: String,
    pub graph_id: String,
}

#[derive(Debug)]
pub struct CreationConfirmed {
  pub config: ProjectConfig,
  pub template: TemplateProject,
  pub output_path: Option<Utf8PathBuf>,
}

#[derive(Debug)]
pub struct ProjectCreated {
    pub config: ProjectConfig,
    pub artifacts: Vec<Utf8PathBuf>,
    pub api_key: String,
}

#[derive(Debug)]
pub struct Completed;