use crate::command::init::states::*;
use camino::Utf8PathBuf;
use std::collections::HashMap;
use std::str::FromStr;

#[test]
fn test_unified_template_processing_fix() {
    // This test verifies that the key issue found in the comparison test is now fixed:
    // Both flows should now handle DOCKER_TAG and other placeholders consistently

    let test_content = r#"
# Test MCP template with critical placeholders
PROJECT_NAME={{PROJECT_NAME}}
APOLLO_KEY={{APOLLO_KEY}}
DOCKER_TAG={{DOCKER_TAG}}
GRAPH_ID=${GRAPH_ID}
VARIANT_NAME=${VARIANT_NAME}
ENDPOINT={{GRAPHQL_ENDPOINT}}
"#;

    let _project_name = "test_project";
    let apollo_key = "service:test-graph:abc123"; // gitleaks:allow
    let graph_ref = rover_client::shared::GraphRef {
        name: "test-graph".to_string(),
        variant: "current".to_string(),
    };

    // Test new state-based approach (should now use unified helper)
    let mcp_creation_confirmed = MCPCreationConfirmed {
        output_path: Utf8PathBuf::from("/tmp/test"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org")
                .unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("test_project")
                .unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str("test-graph")
                .unwrap(),
            project_type: crate::command::init::options::ProjectType::CreateNew,
        },
        composed_template: MCPComposedTemplate {
            base_template: crate::command::init::template_fetcher::Template {
                id: crate::command::init::template_fetcher::TemplateId("test".to_string()),
                display_name: "Test".to_string(),
                path: "test".to_string(),
                language: "TypeScript".to_string(),
                federation_version: "=2.10.0".to_string(),
                max_schema_depth: 5,
                routing_url: "http://localhost:4001".to_string(),
                commands: None,
                start_point_file: "README.md".to_string(),
                print_depth: None,
            },
            merged_files: HashMap::new(),
            merged_files: HashMap::new(),
        },
    };

    let result =
        mcp_creation_confirmed.process_template_placeholders(test_content, apollo_key, &graph_ref);

    println!("=== Original Content ===");
    println!("{}", test_content);

    println!("\n=== Processed Result ===");
    println!("{}", result);

    // Key assertions - these placeholders MUST be replaced
    assert!(
        result.contains("PROJECT_NAME=test_project"),
        "PROJECT_NAME should be replaced"
    );
    assert!(
        result.contains(&format!("APOLLO_KEY={}", apollo_key)),
        "APOLLO_KEY should be replaced"
    );
    assert!(
        result.contains("DOCKER_TAG=test-graph"),
        "DOCKER_TAG should be replaced (this was the main bug!)"
    );
    assert!(
        result.contains("GRAPH_ID=test-graph"),
        "GRAPH_ID should be replaced"
    );
    assert!(
        result.contains("VARIANT_NAME=current"),
        "VARIANT_NAME should be replaced"
    );
    assert!(
        result.contains("ENDPOINT=http://host.docker.internal:4000/graphql"),
        "GRAPHQL_ENDPOINT should be replaced"
    );

    // Ensure placeholders are gone
    assert!(
        !result.contains("{{PROJECT_NAME}}"),
        "PROJECT_NAME placeholder should be removed"
    );
    assert!(
        !result.contains("{{APOLLO_KEY}}"), // gitleaks:allow
        "APOLLO_KEY placeholder should be removed"
    );
    assert!(
        !result.contains("{{DOCKER_TAG}}"),
        "DOCKER_TAG placeholder should be removed"
    );
    assert!(
        !result.contains("${GRAPH_ID}"),
        "GRAPH_ID placeholder should be removed"
    );
    assert!(
        !result.contains("${VARIANT_NAME}"),
        "VARIANT_NAME placeholder should be removed" // gitleaks:allow
    );
    assert!(
        !result.contains("{{GRAPHQL_ENDPOINT}}"),
        "GRAPHQL_ENDPOINT placeholder should be removed"
    );

    println!(
        "\n✅ Template processing fix verified - all critical placeholders are correctly replaced!"
    );
}

#[test]
fn test_unified_helper_directly() {
    // Test the unified helper function directly to ensure it handles all placeholders

    let test_content =
        "PROJECT={{PROJECT_NAME}} KEY={{APOLLO_KEY}} TAG={{DOCKER_TAG}} ID=${GRAPH_ID}";

    let graph_ref = rover_client::shared::GraphRef {
        name: "my-graph".to_string(),
        variant: "current".to_string(),
    };

    let ctx = crate::command::init::helpers::MCPTemplateContext {
        project_name: "my_project",
        graph_id: "my-graph",
        graph_name: "my_project",
        variant_name: "current",
        organization_name: "my_org",
        api_key: "service:key:123", // gitleaks:allow
        graph_ref: &graph_ref,
        mcp_server_binary: None,
        mcp_config_path: None,
        tools_path: None,
    };

    let result =
        crate::command::init::helpers::process_mcp_template_placeholders(test_content, &ctx);

    println!("Direct helper test result: {}", result);

    // All placeholders should be replaced
    assert!(result.contains("PROJECT=my_project"));
    assert!(result.contains("KEY=service:key:123"));
    assert!(result.contains("TAG=my-graph")); // This is the critical DOCKER_TAG fix
    assert!(result.contains("ID=my-graph"));

    // No placeholders should remain
    assert!(!result.contains("{{PROJECT_NAME}}"));
    assert!(!result.contains("{{APOLLO_KEY}}"));
    assert!(!result.contains("{{DOCKER_TAG}}"));
    assert!(!result.contains("${GRAPH_ID}"));
}
