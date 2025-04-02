use crate::command::init::config::ProjectConfig;
use crate::options::{ProjectType, ProjectUseCase};

// 1. Initial state
#[derive(Debug)]
pub struct Welcome;

// 2. After selecting "Create a new GraphQL API" or "Add a subgraph"
#[derive(Debug)]
pub struct ProjectTypeSelected {
    pub project_type: ProjectType,
}

// 3. After selecting organization
#[derive(Debug)]
pub struct OrganizationSelected {
    pub project_type: ProjectType,
    pub organization: String,
}

// 4. After selecting use case
#[derive(Debug)]
pub struct UseCaseSelected {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
}

// 5. After entering project name
#[derive(Debug)]
pub struct ProjectNamed {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: String,
}

// 6. After confirming graph ID
#[derive(Debug)]
pub struct GraphIdConfirmed {
    pub project_type: ProjectType,
    pub organization: String,
    pub use_case: ProjectUseCase,
    pub project_name: String,
    pub graph_id: String,
}

// 7. After displaying what will be created and getting confirmation
#[derive(Debug)]
pub struct CreationConfirmed {
    pub config: ProjectConfig,
    pub artifacts: Vec<String>,
}

// 8. After files are created and API call is completed
#[derive(Debug)]
pub struct ProjectCreated {
    pub config: ProjectConfig,
    pub artifacts: Vec<String>,
    pub api_key: String,
}

// 9. Final state
#[derive(Debug)]
pub struct Completed;