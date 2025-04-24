#[cfg(feature = "composition-js")]
use crate::command::init::graph_id::validation::GraphId;
#[cfg(feature = "composition-js")]
use crate::command::init::options::{OrganizationId, ProjectName, ProjectType, ProjectUseCase};
#[cfg(feature = "composition-js")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "composition-js")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: ProjectType,
    pub organization: OrganizationId,
    pub use_case: ProjectUseCase,
    pub project_name: ProjectName,
    pub graph_id: GraphId,
}
