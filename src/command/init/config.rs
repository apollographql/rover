use crate::options::{ProjectType, ProjectUseCase};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: String,
    pub graph_id: String,
}

impl ProjectConfig {
  // TODO
}