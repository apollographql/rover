use crate::command::init::graph_id::validation::GraphId;
use crate::command::init::options::{
    OrganizationId, ProjectName, ProjectType, ProjectUseCase, SchemaName,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: GraphId,
    pub schema_name: SchemaName,
}
