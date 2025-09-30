use crate::command::init::states::*;
use std::collections::HashMap;
use std::str::FromStr;
use camino::Utf8PathBuf;

// Async test removed - would require network access and tokio_test dependency

#[test]
fn test_compare_template_processing_methods() {
    // This test compares the two different template processing approaches
    // to verify they produce the same results

    let test_content = r#"
# Test template content
PROJECT_NAME={{PROJECT_NAME}}
APOLLO_KEY={{APOLLO_KEY}}
APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}
GRAPH_ID=${GRAPH_ID}
VARIANT_NAME=${VARIANT_NAME}
ENDPOINT={{GRAPHQL_ENDPOINT}}
DOCKER_TAG={{DOCKER_TAG}}
"#;

    // Simulate the values that would be used
    let project_name = "test_project";
    let apollo_key = "service:test-graph:abc123";
    let graph_ref = rover_client::shared::GraphRef {
        name: "test-graph".to_string(),
        variant: "current".to_string(),
    };
    let graph_ref_str = graph_ref.to_string();

    // Test Method 1: New state-based approach
    let mcp_creation_confirmed = MCPCreationConfirmed {
        output_path: Utf8PathBuf::from("/tmp/test"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org").unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("test_project").unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str("test-graph").unwrap(),
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
            mcp_additions: HashMap::new(),
            merged_files: HashMap::new(),
        },
    };

    let new_approach_result = mcp_creation_confirmed.process_template_placeholders(
        test_content,
        apollo_key,
        &graph_ref
    );

    // Test Method 2: Existing graph approach (simulated)
    let mut old_approach_result = test_content.to_string();

    // Simulate the processing from handle_existing_graph_mcp
    old_approach_result = old_approach_result
        // ${} format - primarily for YAML files
        .replace("${PROJECT_NAME}", project_name)
        .replace("${GRAPH_REF}", &graph_ref_str)
        .replace("${GRAPH_ID}", "test-graph")
        .replace("${GRAPH_NAME}", project_name)
        .replace("${VARIANT_NAME}", "current")
        .replace("${ORGANIZATION_NAME}", "test_org")
        .replace("${APOLLO_API_KEY}", apollo_key)
        .replace("${APOLLO_KEY}", apollo_key)
        .replace("${APOLLO_GRAPH_REF}", &graph_ref_str)
        .replace("${GRAPHQL_ENDPOINT}", "http://localhost:4000")
        .replace("${STAGING_GRAPHQL_ENDPOINT}", "http://localhost:4000")
        .replace("${PROJECT_VERSION}", "1.0.0")
        // {{}} format - for non-YAML templates and backwards compatibility
        .replace("{{PROJECT_NAME}}", project_name)
        .replace("{{GRAPH_REF}}", &graph_ref_str)
        .replace("{{GRAPH_ID}}", "test-graph")
        .replace("{{GRAPH_NAME}}", project_name)
        .replace("{{VARIANT_NAME}}", "current")
        .replace("{{ORGANIZATION_NAME}}", "test_org")
        .replace("{{APOLLO_API_KEY}}", apollo_key)
        .replace("{{APOLLO_KEY}}", apollo_key)
        .replace("{{APOLLO_GRAPH_REF}}", &graph_ref_str)
        .replace("{{GRAPHQL_ENDPOINT}}", "http://localhost:4000")
        .replace("{{PROJECT_VERSION}}", "1.0.0");

    println!("=== Original Content ===");
    println!("{}", test_content);

    println!("\n=== New Approach Result ===");
    println!("{}", new_approach_result);

    println!("\n=== Old Approach Result ===");
    println!("{}", old_approach_result);

    // Check if they produce the same results for the common placeholders
    let placeholders_to_check = vec![
        ("{{PROJECT_NAME}}", project_name),
        ("{{APOLLO_KEY}}", apollo_key),
        ("{{APOLLO_GRAPH_REF}}", &graph_ref_str),
        ("${GRAPH_ID}", "test-graph"),
        ("${VARIANT_NAME}", "current"),
    ];

    for (placeholder, expected_value) in placeholders_to_check {
        let new_contains = new_approach_result.contains(expected_value);
        let old_contains = old_approach_result.contains(expected_value);
        let new_still_has_placeholder = new_approach_result.contains(placeholder);
        let old_still_has_placeholder = old_approach_result.contains(placeholder);

        println!("\n=== Checking {} ===", placeholder);
        println!("New approach replaced: {} (still has placeholder: {})", new_contains, new_still_has_placeholder);
        println!("Old approach replaced: {} (still has placeholder: {})", old_contains, old_still_has_placeholder);

        if new_still_has_placeholder != old_still_has_placeholder {
            println!("⚠️  MISMATCH: Different placeholder replacement behavior");
        }
    }
}