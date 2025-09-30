#[cfg(test)]
mod tests {
    use crate::command::init::options::{
        OrganizationId, ProjectName, ProjectNameOpt, ProjectTemplateOpt, ProjectType,
    };
    use crate::command::init::states::*;
    use crate::command::init::template_fetcher::{Template, TemplateId};
    use camino::Utf8PathBuf;
    use std::collections::HashMap;

    #[test]
    fn test_project_type_selected_mcp_initialization() {
        let project_type_selected = ProjectTypeSelected {
            project_type: ProjectType::CreateNew,
            output_path: ".".into(),
        };

        // Test with MCP flag enabled
        let mcp_options = ProjectTemplateOpt {
            template: None,
            mcp: true,
        };

        let result = project_type_selected.initialize_mcp_augmentation(&mcp_options);
        assert!(result.is_ok());

        let mcp_init = result.unwrap();
        assert_eq!(mcp_init.project_type, ProjectType::CreateNew);
        assert_eq!(mcp_init.output_path, Utf8PathBuf::from("."));
    }

    #[test]
    fn test_project_type_selected_mcp_initialization_without_flag() {
        let project_type_selected = ProjectTypeSelected {
            project_type: ProjectType::CreateNew,
            output_path: ".".into(),
        };

        // Test without MCP flag (should fail)
        let non_mcp_options = ProjectTemplateOpt {
            template: None,
            mcp: false,
        };

        let result = project_type_selected.initialize_mcp_augmentation(&non_mcp_options);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("MCP initialization called without --mcp flag")
        );
    }

    #[test]
    fn test_mcp_template_composed_enter_project_name() {
        let mcp_template_composed = MCPTemplateComposed {
            output_path: ".".into(),
            project_type: ProjectType::CreateNew,
            organization: "test-org".parse::<OrganizationId>().unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            composed_template: create_mock_mcp_composed_template(),
            setup_type: MCPSetupType::NewProject,
            data_source_type: MCPDataSourceType::ExternalAPIs,
        };

        let name_options = ProjectNameOpt {
            project_name: Some("test-mcp-project".parse::<ProjectName>().unwrap()),
        };

        let result = mcp_template_composed.enter_project_name(&name_options);
        assert!(result.is_ok());

        let mcp_named = result.unwrap();
        assert_eq!(mcp_named.project_name.to_string(), "test-mcp-project");
        assert_eq!(mcp_named.organization.to_string(), "test-org");
    }

    #[test]
    fn test_mcp_graph_id_confirmed_creates_config() {
        let mcp_graph_confirmed = MCPGraphIdConfirmed {
            output_path: ".".into(),
            project_type: ProjectType::CreateNew,
            organization: "test-org".parse::<OrganizationId>().unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: "test-project".parse::<ProjectName>().unwrap(),
            graph_id: "test-graph-id".parse().unwrap(),
            composed_template: create_mock_mcp_composed_template(),
            setup_type: MCPSetupType::NewProject,
            data_source_type: MCPDataSourceType::ExternalAPIs,
        };

        // Test that the state can properly construct a ProjectConfig
        let config = crate::command::init::config::ProjectConfig {
            organization: mcp_graph_confirmed.organization.clone(),
            use_case: mcp_graph_confirmed.use_case,
            project_name: mcp_graph_confirmed.project_name.clone(),
            graph_id: mcp_graph_confirmed.graph_id.clone(),
            project_type: mcp_graph_confirmed.project_type,
        };

        assert_eq!(config.project_name.to_string(), "test-project");
        assert_eq!(config.graph_id.to_string(), "test-graph-id");
        assert_eq!(config.organization.to_string(), "test-org");
        assert_eq!(mcp_graph_confirmed.output_path, Utf8PathBuf::from("."));
    }

    #[test]
    fn test_mcp_composed_template_new() {
        let base_template = create_mock_template();
        let mut pre_merged_files = HashMap::new();
        pre_merged_files.insert(
            Utf8PathBuf::from(".apollo/mcp.local.yaml"),
            b"operations:\n  source: collection".to_vec(),
        );

        let composed = MCPComposedTemplate::new(base_template.clone(), pre_merged_files.clone());

        assert_eq!(composed.base_template.id, base_template.id);
        assert_eq!(composed.merged_files, pre_merged_files);
        assert!(
            composed
                .merged_files
                .contains_key(&Utf8PathBuf::from(".apollo/mcp.local.yaml"))
        );
    }

    #[test]
    fn test_mcp_composed_template_list_files() {
        let composed = create_mock_mcp_composed_template();
        let files = composed.list_files();

        assert!(!files.is_empty());
        assert!(files.contains(&Utf8PathBuf::from(".apollo/mcp.local.yaml")));
    }

    // Helper functions for tests
    fn create_mock_template() -> Template {
        Template {
            id: TemplateId("connectors".to_string()),
            display_name: "Start with REST".to_string(),
            path: "start-with-rest".to_string(),
            language: "GraphQL".to_string(),
            federation_version: "=2.10.0".to_string(),
            max_schema_depth: 5,
            routing_url: "http://ignore".to_string(),
            commands: None,
            start_point_file: "GETTING_STARTED.MD".to_string(),
            print_depth: None,
        }
    }

    fn create_mock_mcp_composed_template() -> MCPComposedTemplate {
        let base_template = create_mock_template();
        let mut pre_merged_files = HashMap::new();
        pre_merged_files.insert(
            Utf8PathBuf::from(".apollo/mcp.local.yaml"),
            b"operations:\n  source: collection\n  id: default".to_vec(),
        );
        pre_merged_files.insert(
            Utf8PathBuf::from("mcp.Dockerfile"),
            b"FROM node:18\nRUN npm install".to_vec(),
        );

        MCPComposedTemplate::new(base_template, pre_merged_files)
    }
}
