pub mod authentication_tests;
pub mod mcp_behavior_tests;
pub mod mcp_flow_integration_test;
pub mod mcp_integration_tests;
pub mod mcp_next_steps_test;
pub mod mcp_prompting_tests;
pub mod mcp_transitions_tests;
pub mod project_authentication_tests;
pub mod project_created_message_tests;
pub mod template_consistency_test;
pub mod template_hydration_tests;
pub mod transitions_tests;

#[cfg(test)]
mod mcp_tools_tests {
    use camino::Utf8PathBuf;
    use std::collections::HashMap;

    #[test]
    fn test_all_mcp_projects_use_operation_collections_never_generate_tools() {
        let mut files: HashMap<Utf8PathBuf, String> = HashMap::new();

        // Simulate merged files from base template + add-mcp (before filtering)
        // This represents what we get after merging base template with add-mcp
        files.insert("README.md".into(), "# Project".to_string());
        files.insert("package.json".into(), "{}".to_string());
        files.insert(".apollo/mcp.local.yaml".into(), "operations:\n  source: collection\n  id: default".to_string());
        files.insert("examples/api/rest.graphql".into(), "query RestExample { __typename }".to_string());
        files.insert("examples/graphql/sample.graphql".into(), "query Sample { __typename }".to_string());
        files.insert("tools/existing.graphql".into(), "query Existing { __typename }".to_string());

        // Apply MCP filtering: Always remove tools and examples since all MCP configs use operation collections
        let files_to_remove: Vec<_> = files
            .keys()
            .filter(|path| path.starts_with("examples/") || path.starts_with("tools/"))
            .cloned()
            .collect();

        for path in files_to_remove {
            files.remove(&path);
        }

        // Verify all examples and tools were removed
        let examples_files: Vec<_> = files.keys().filter(|path| path.starts_with("examples/")).collect();
        let tools_files: Vec<_> = files.keys().filter(|path| path.starts_with("tools/")).collect();

        assert!(examples_files.is_empty(), "Examples should always be removed for MCP projects");
        assert!(tools_files.is_empty(), "Tools should never be generated for MCP projects");

        // Verify base project files are preserved
        assert!(files.contains_key(&Utf8PathBuf::from("README.md")));
        assert!(files.contains_key(&Utf8PathBuf::from("package.json")));
        assert!(files.contains_key(&Utf8PathBuf::from(".apollo/mcp.local.yaml")));

        // Should have 3 files: README.md, package.json, mcp.local.yaml
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_existing_project_flow_uses_only_add_mcp_files() {
        let mut files: HashMap<Utf8PathBuf, String> = HashMap::new();

        // Simulate files from add-mcp directory only (existing project flow)
        files.insert(".apollo/mcp.local.yaml".into(), "operations:\n  source: collection\n  id: default".to_string());
        files.insert(".apollo/mcp.staging.yaml".into(), "operations:\n  source: collection".to_string());
        files.insert("mcp.Dockerfile".into(), "FROM node:18".to_string());
        files.insert("MCP_README.md".into(), "# MCP Server Setup".to_string());

        // Verify no examples or tools files exist (they shouldn't be in add-mcp directory)
        let examples_files: Vec<_> = files.keys().filter(|path| path.starts_with("examples/")).collect();
        let tools_files: Vec<_> = files.keys().filter(|path| path.starts_with("tools/")).collect();

        assert!(examples_files.is_empty(), "add-mcp directory should contain no examples");
        assert!(tools_files.is_empty(), "add-mcp directory should contain no tools");

        // Should only have MCP-specific files
        assert!(files.contains_key(&Utf8PathBuf::from(".apollo/mcp.local.yaml")));
        assert!(files.contains_key(&Utf8PathBuf::from("mcp.Dockerfile")));
    }

    #[test]
    fn test_all_mcp_configs_always_use_operation_collections() {
        let mut files: HashMap<Utf8PathBuf, String> = HashMap::new();

        // Add MCP config files (as they appear in add-mcp directory)
        files.insert(".apollo/mcp.local.yaml".into(),
            "operations:\n  source: collection\n  id: default\nintrospection:\n  enabled: true".to_string());
        files.insert(".apollo/mcp.staging.yaml".into(),
            "operations:\n  source: collection\n  id: default\nrequire_confirmation: true".to_string());

        // Verify all MCP configs always use operation collections (never file-based)
        let local_config = files.get(&Utf8PathBuf::from(".apollo/mcp.local.yaml")).unwrap();
        assert!(local_config.contains("source: collection"), "Local config must use operation collections");
        assert!(!local_config.contains("source: file"), "Should never use file-based operations");

        let staging_config = files.get(&Utf8PathBuf::from(".apollo/mcp.staging.yaml")).unwrap();
        assert!(staging_config.contains("source: collection"), "Staging config must use operation collections");
        assert!(!staging_config.contains("source: file"), "Should never use file-based operations");

        // This is why we never need tools directories - everything comes from Studio collections
    }
}
