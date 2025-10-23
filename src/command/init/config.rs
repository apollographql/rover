use serde::{Deserialize, Serialize};

use crate::command::init::{
    graph_id::validation::GraphId,
    options::{OrganizationId, ProjectName, ProjectType, ProjectUseCase},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: GraphId,
}
