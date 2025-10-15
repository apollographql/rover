use std::collections::HashMap;

use crate::command::init::config::ProjectConfig;
use crate::command::init::graph_id::GraphId;
use crate::command::init::options::{
    OrganizationId, ProjectName, ProjectNameOpt, ProjectOrganizationOpt, ProjectType,
    ProjectTypeOpt, ProjectUseCase, ProjectUseCaseOpt,
};
use crate::command::init::states::*;
use crate::command::init::template_fetcher::{Template, TemplateId};
use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use camino::Utf8PathBuf;

mod mock {
    use super::*;

    #[derive(Clone, Default)]
    pub struct MockHttpService {}

    #[derive(Clone)]
    pub struct MockTemplateFetcher {
        pub files: Vec<String>,
    }

    impl MockTemplateFetcher {
        pub fn new(_http_service: MockHttpService) -> Self {
            Self {
                files: vec![
                    "file1.txt".to_string(),
                    "file2.txt".to_string(),
                    "schema.graphql".to_string(),
                ],
            }
        }

        pub fn list_files(&self) -> RoverResult<Vec<String>> {
            Ok(self.files.clone())
        }
    }

    #[derive(Default)]
    pub struct MockTemplateOperations {}

    impl MockTemplateOperations {
        pub fn prompt_creation(_artifacts: Vec<String>) -> RoverResult<bool> {
            Ok(true)
        }
    }

    #[allow(dead_code)]
    pub struct MockCreationConfirmed {
        pub config: ProjectConfig,
        pub template_fetcher: MockTemplateFetcher,
        pub output_path: Option<Utf8PathBuf>,
    }
}

#[test]
fn test_project_type_selected_transition() {
    let project_type_selected = ProjectTypeSelected {
        project_type: ProjectType::CreateNew,
        output_path: ".".into(),
    };

    let options = ProjectOrganizationOpt {
        organization: "test-org".parse::<OrganizationId>().ok(),
    };

    let organizations = ["test-org".to_string(), "other-org".to_string()];

    let result: RoverResult<OrganizationSelected> = {
        let organization = options.get_organization().unwrap();
        if organizations.contains(&organization.to_string()) {
            Ok(OrganizationSelected {
                project_type: project_type_selected.project_type.clone(),
                organization,
                output_path: ".".into(),
            })
        } else {
            Err(RoverError::new(anyhow!("Organization not found")))
        }
    };

    assert!(result.is_ok());
    let next_state = result.unwrap();
    assert_eq!(next_state.project_type, ProjectType::CreateNew);
    assert_eq!(
        next_state.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
}

#[test]
fn test_organization_selected_transition() {
    let org_selected = OrganizationSelected {
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        output_path: ".".into(),
    };

    let options = ProjectUseCaseOpt {
        project_use_case: Some(ProjectUseCase::Connectors),
    };

    let result: RoverResult<UseCaseSelected> = {
        let use_case = options.project_use_case.clone().unwrap();
        Ok(UseCaseSelected {
            output_path: ".".into(),
            project_type: org_selected.project_type.clone(),
            organization: org_selected.organization.clone(),
            use_case,
        })
    };

    assert!(result.is_ok());
    let next_state = result.unwrap();
    assert_eq!(next_state.project_type, ProjectType::CreateNew);
    assert_eq!(
        next_state.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
    assert_eq!(next_state.use_case, ProjectUseCase::Connectors);
}

#[test]
fn test_use_case_selected_transition() {
    let use_case_selected = UseCaseSelected {
        output_path: ".".into(),
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        use_case: ProjectUseCase::Connectors,
    };

    let options = ProjectNameOpt {
        project_name: Some("test-graph".parse::<ProjectName>().unwrap()),
    };

    let result: RoverResult<ProjectNamed> = {
        let project_name = options.project_name.ok_or(|| {}).ok().unwrap();
        Ok(ProjectNamed {
            output_path: ".".into(),
            project_type: use_case_selected.project_type.clone(),
            organization: use_case_selected.organization.clone(),
            use_case: use_case_selected.use_case.clone(),
            project_name,
            selected_template: SelectedTemplateState {
                template: Template {
                    id: TemplateId("connectors".to_string()),
                    display_name: "Start with REST".to_string(),
                    path: "start-with-rest".to_string(),
                    language: "GraphQL".to_string(),
                    federation_version: "=2.10.0".to_string(),
                    max_schema_depth: 5,
                    routing_url: "http://ignore".to_string(),
                    commands: None,
                    start_point_file: "GETTING_STARTED.md".to_string(),
                    print_depth: None,
                },
                files: HashMap::new(),
            },
        })
    };

    assert!(result.is_ok());
    let next_state = result.unwrap();
    assert_eq!(next_state.project_type, ProjectType::CreateNew);
    assert_eq!(
        next_state.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
    assert_eq!(next_state.use_case, ProjectUseCase::Connectors);
    assert_eq!(
        next_state.project_name,
        "test-graph".parse::<ProjectName>().unwrap()
    );
}

#[test]
fn test_project_named_transition() {
    let project_named = ProjectNamed {
        output_path: ".".into(),
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        use_case: ProjectUseCase::Connectors,
        selected_template: SelectedTemplateState {
            template: Template {
                id: TemplateId("connectors".to_string()),
                display_name: "Start with REST".to_string(),
                path: "start-with-rest".to_string(),
                language: "GraphQL".to_string(),
                federation_version: "=2.10.0".to_string(),
                max_schema_depth: 5,
                routing_url: "http://ignore".to_string(),
                commands: None,
                start_point_file: "GETTING_STARTED.md".to_string(),
                print_depth: None,
            },
            files: HashMap::new(),
        },
        project_name: "test-graph".parse().unwrap(),
    };

    let result: RoverResult<GraphIdConfirmed> = {
        let graph_id = "test-graph-id".parse::<GraphId>().unwrap();
        Ok(GraphIdConfirmed {
            output_path: ".".into(),
            project_type: project_named.project_type.clone(),
            organization: project_named.organization.clone(),
            use_case: project_named.use_case.clone(),
            project_name: project_named.project_name.clone(),
            selected_template: project_named.selected_template,
            graph_id,
        })
    };

    assert!(result.is_ok());
    let next_state = result.unwrap();
    assert_eq!(next_state.project_type, ProjectType::CreateNew);
    assert_eq!(
        next_state.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
    assert_eq!(next_state.use_case, ProjectUseCase::Connectors);
    assert_eq!(next_state.project_name, "test-graph".parse().unwrap());
    assert_eq!(
        next_state.graph_id,
        "test-graph-id".parse::<GraphId>().unwrap()
    );
}

#[test]
fn test_graph_id_confirmed_config() {
    let graph_id_confirmed = GraphIdConfirmed {
        output_path: ".".into(),
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        use_case: ProjectUseCase::Connectors,
        project_name: "test-graph".parse().unwrap(),
        graph_id: "test-graph-id".parse::<GraphId>().unwrap(),
        selected_template: SelectedTemplateState {
            template: Template {
                id: TemplateId("connectors".to_string()),
                display_name: "Start with REST".to_string(),
                path: "start-with-rest".to_string(),
                language: "GraphQL".to_string(),
                federation_version: "=2.10.0".to_string(),
                max_schema_depth: 5,
                routing_url: "http://ignore".to_string(),
                commands: None,
                start_point_file: "GETTING_STARTED.md".to_string(),
                print_depth: None,
            },
            files: HashMap::new(),
        },
    };

    let config = ProjectConfig {
        project_type: graph_id_confirmed.project_type.clone(),
        organization: graph_id_confirmed.organization.clone(),
        use_case: graph_id_confirmed.use_case.clone(),
        project_name: graph_id_confirmed.project_name,
        graph_id: graph_id_confirmed.graph_id.clone(),
    };

    assert_eq!(config.project_type, ProjectType::CreateNew);
    assert_eq!(
        config.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
    assert_eq!(config.use_case, ProjectUseCase::Connectors);
    assert_eq!(
        config.project_name,
        "test-graph".parse::<ProjectName>().unwrap()
    );
    assert_eq!(config.graph_id, "test-graph-id".parse::<GraphId>().unwrap());
}

#[tokio::test]
async fn test_graph_id_confirmed_preview_for_connectors() {
    let graph_id_confirmed = GraphIdConfirmed {
        output_path: ".".into(),
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        use_case: ProjectUseCase::Connectors,
        project_name: "test-graph".parse().unwrap(),
        graph_id: "test-graph-id".parse::<GraphId>().unwrap(),
        selected_template: SelectedTemplateState {
            template: Template {
                id: TemplateId("connectors".to_string()),
                display_name: "Start with REST".to_string(),
                path: "start-with-rest".to_string(),
                language: "GraphQL".to_string(),
                federation_version: "=2.10.0".to_string(),
                max_schema_depth: 5,
                routing_url: "http://ignore".to_string(),
                commands: None,
                start_point_file: "GETTING_STARTED.md".to_string(),
                print_depth: None,
            },
            files: HashMap::new(),
        },
    };

    let http_service = mock::MockHttpService::default();

    let result: RoverResult<Option<mock::MockCreationConfirmed>> = async {
        let config = ProjectConfig {
            project_type: graph_id_confirmed.project_type.clone(),
            organization: graph_id_confirmed.organization.clone(),
            use_case: graph_id_confirmed.use_case.clone(),
            project_name: graph_id_confirmed.project_name,
            graph_id: graph_id_confirmed.graph_id.clone(),
        };

        let template_fetcher = mock::MockTemplateFetcher::new(http_service);

        let artifacts = template_fetcher.list_files()?;
        let confirmed = mock::MockTemplateOperations::prompt_creation(artifacts.clone())?;

        if confirmed {
            Ok(Some(mock::MockCreationConfirmed {
                config,
                template_fetcher,
                output_path: None,
            }))
        } else {
            Ok(None)
        }
    }
    .await;

    assert!(result.is_ok());
    let next_state_option = result.unwrap();
    assert!(next_state_option.is_some());
    let next_state = next_state_option.unwrap();
    assert_eq!(next_state.config.project_type, ProjectType::CreateNew);
    assert_eq!(
        next_state.config.organization,
        "test-org".parse::<OrganizationId>().unwrap()
    );
    assert_eq!(next_state.config.use_case, ProjectUseCase::Connectors);
    assert_eq!(
        next_state.config.project_name,
        "test-graph".parse::<ProjectName>().unwrap()
    );
    assert_eq!(
        next_state.config.graph_id,
        "test-graph-id".parse::<GraphId>().unwrap()
    );
}

#[tokio::test]
async fn test_graph_id_confirmed_preview_for_graphql_template() {
    let graph_id_confirmed = GraphIdConfirmed {
        project_type: ProjectType::CreateNew,
        organization: "test-org".parse::<OrganizationId>().unwrap(),
        use_case: ProjectUseCase::GraphQLTemplate,
        project_name: "test-graph".parse::<ProjectName>().unwrap(),
        graph_id: "test-graph-id".parse::<GraphId>().unwrap(),
        output_path: ".".into(),
        selected_template: SelectedTemplateState {
            template: Template {
                id: TemplateId("connectors".to_string()),
                display_name: "Start with REST".to_string(),
                path: "start-with-rest".to_string(),
                language: "GraphQL".to_string(),
                federation_version: "=2.10.0".to_string(),
                max_schema_depth: 5,
                routing_url: "http://ignore".to_string(),
                commands: None,
                start_point_file: "GETTING_STARTED.md".to_string(),
                print_depth: None,
            },
            files: HashMap::new(),
        },
    };

    let http_service = mock::MockHttpService::default();

    let result: RoverResult<Option<mock::MockCreationConfirmed>> = async {
        if graph_id_confirmed.use_case == ProjectUseCase::GraphQLTemplate {
            return Ok(None);
        }

        let config = ProjectConfig {
            project_type: graph_id_confirmed.project_type.clone(),
            organization: graph_id_confirmed.organization.clone(),
            use_case: graph_id_confirmed.use_case.clone(),
            project_name: graph_id_confirmed.project_name.clone(),
            graph_id: graph_id_confirmed.graph_id.clone(),
        };

        let template_fetcher = mock::MockTemplateFetcher::new(http_service);

        Ok(Some(mock::MockCreationConfirmed {
            config,
            template_fetcher,
            output_path: None,
        }))
    }
    .await;

    assert!(result.is_ok());
    let next_state_option = result.unwrap();
    assert!(next_state_option.is_none());
}

#[test]
fn test_project_type_dialog() {
    let options = ProjectTypeOpt::default();

    assert_eq!(options.get_project_type(), None);

    let options_with_value = ProjectTypeOpt {
        project_type: Some(ProjectType::CreateNew),
    };
    assert_eq!(
        options_with_value.get_project_type(),
        Some(ProjectType::CreateNew)
    );
}
