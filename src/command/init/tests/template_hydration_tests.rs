use crate::command::init::graph_id::validation::GraphId;
use crate::command::init::options::*;
use crate::command::init::states::*;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use std::collections::HashMap;
use std::str::FromStr;

#[test]
fn test_mcp_template_placeholder_processing() {
    // Create a sample template with MCP placeholders
    let mut files = HashMap::new();

    // Add an .env.template file with placeholders
    files.insert(
        Utf8PathBuf::from(".env.template"),
        r#"APOLLO_KEY={{APOLLO_KEY}}
APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}
PROJECT_NAME={{PROJECT_NAME}}
GRAPHQL_ENDPOINT={{GRAPHQL_ENDPOINT}}
"#
        .as_bytes()
        .to_vec(),
    );

    // Add a YAML file with ${} placeholders
    files.insert(
        Utf8PathBuf::from(".apollo/mcp.local.yaml"),
        r#"# MCP Configuration
project: ${PROJECT_NAME}
apollo_key: ${APOLLO_KEY}
graph_ref: ${APOLLO_GRAPH_REF}
endpoint: ${GRAPHQL_ENDPOINT}
"#
        .as_bytes()
        .to_vec(),
    );

    // Add a Docker file with mixed placeholders
    files.insert(
        Utf8PathBuf::from("mcp.Dockerfile"),
        r#"FROM node:18
WORKDIR /app
COPY . .
ENV PROJECT_NAME={{PROJECT_NAME}}
ENV APOLLO_KEY={{APOLLO_KEY}}
RUN echo "Building {{PROJECT_NAME}}"
"#
        .as_bytes()
        .to_vec(),
    );

    let composed_template = MCPComposedTemplate {
        base_template: crate::command::init::template_fetcher::Template {
            id: crate::command::init::template_fetcher::TemplateId("test".to_string()),
            display_name: "Test Template".to_string(),
            path: "test".to_string(),
            language: "TypeScript".to_string(),
            federation_version: "=2.10.0".to_string(),
            max_schema_depth: 5,
            routing_url: "http://localhost:4001".to_string(),
            commands: None,
            start_point_file: "README.md".to_string(),
            print_depth: None,
        },
        merged_files: files,
        agents_merged_into_readme: false,
    };

    let mcp_creation_confirmed = MCPCreationConfirmed {
        output_path: Utf8PathBuf::from("/tmp/test"),
        config: crate::command::init::config::ProjectConfig {
            organization: OrganizationId::from_str("test_org").unwrap(),
            use_case: ProjectUseCase::Connectors,
            project_name: ProjectName::from_str("my_test_project").unwrap(),
            graph_id: GraphId::from_str("my-test-graph").unwrap(),
            project_type: ProjectType::CreateNew,
        },
        composed_template,
    };

    // Test the template placeholder processing
    let test_api_key = "service:my-test-graph:abc123def456"; // gitleaks:allow
    let test_graph_ref = GraphRef {
        name: "my-test-graph".to_string(),
        variant: "current".to_string(),
    };

    // Process .env.template content
    let env_content = r#"APOLLO_KEY={{APOLLO_KEY}}
APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}
PROJECT_NAME={{PROJECT_NAME}}
GRAPHQL_ENDPOINT={{GRAPHQL_ENDPOINT}}
"#;

    let processed_env = mcp_creation_confirmed.process_template_placeholders(
        env_content,
        test_api_key,
        &test_graph_ref,
    );

    println!("=== Original .env.template content ===");
    println!("{}", env_content);
    println!("=== Processed .env content ===");
    println!("{}", processed_env);

    // Verify the placeholders were replaced
    assert!(processed_env.contains(&format!("APOLLO_KEY={}", test_api_key)));
    assert!(processed_env.contains(&format!("APOLLO_GRAPH_REF={}", test_graph_ref)));
    assert!(processed_env.contains("PROJECT_NAME=my_test_project"));
    assert!(processed_env.contains("GRAPHQL_ENDPOINT=http://host.docker.internal:4000/graphql"));

    // Verify placeholders are no longer present
    assert!(!processed_env.contains("{{APOLLO_KEY}}"));
    assert!(!processed_env.contains("{{APOLLO_GRAPH_REF}}"));
    assert!(!processed_env.contains("{{PROJECT_NAME}}"));
    assert!(!processed_env.contains("{{GRAPHQL_ENDPOINT}}"));

    // Test YAML file with ${} format
    let yaml_content = r#"# MCP Configuration
project: ${PROJECT_NAME}
apollo_key: ${APOLLO_KEY}
graph_ref: ${APOLLO_GRAPH_REF}
endpoint: ${GRAPHQL_ENDPOINT}
"#;

    let processed_yaml = mcp_creation_confirmed.process_template_placeholders(
        yaml_content,
        test_api_key,
        &test_graph_ref,
    );

    println!("=== Original YAML content ===");
    println!("{}", yaml_content);
    println!("=== Processed YAML content ===");
    println!("{}", processed_yaml);

    // Verify ${} placeholders were replaced
    assert!(processed_yaml.contains("project: my_test_project"));
    assert!(processed_yaml.contains(&format!("apollo_key: {}", test_api_key)));
    assert!(processed_yaml.contains(&format!("graph_ref: {}", test_graph_ref)));
    assert!(processed_yaml.contains("endpoint: http://localhost:4000/graphql"));

    // Verify ${} placeholders are no longer present
    assert!(!processed_yaml.contains("${PROJECT_NAME}"));
    assert!(!processed_yaml.contains("${APOLLO_KEY}"));
    assert!(!processed_yaml.contains("${APOLLO_GRAPH_REF}"));
    assert!(!processed_yaml.contains("${GRAPHQL_ENDPOINT}"));
}

#[test]
fn test_mcp_template_file_processing_in_create_project() {
    // This test simulates what happens during create_project() to verify
    // that template files get their placeholders replaced correctly

    let mut files = HashMap::new();
    files.insert(
        Utf8PathBuf::from(".env.template"),
        r#"# Test env file
APOLLO_KEY="{{APOLLO_KEY}}"
PROJECT_NAME="{{PROJECT_NAME}}"
"#
        .as_bytes()
        .to_vec(),
    );

    files.insert(
        Utf8PathBuf::from("README.md"),
        r#"# {{PROJECT_NAME}}

This is a test project with graph ref: {{APOLLO_GRAPH_REF}}
"#
        .as_bytes()
        .to_vec(),
    );

    let composed_template = MCPComposedTemplate {
        base_template: crate::command::init::template_fetcher::Template {
            id: crate::command::init::template_fetcher::TemplateId("test".to_string()),
            display_name: "Test Template".to_string(),
            path: "test".to_string(),
            language: "TypeScript".to_string(),
            federation_version: "=2.10.0".to_string(),
            max_schema_depth: 5,
            routing_url: "http://localhost:4001".to_string(),
            commands: None,
            start_point_file: "README.md".to_string(),
            print_depth: None,
        },
        merged_files: files,
        agents_merged_into_readme: false,
    };

    let mcp_creation_confirmed = MCPCreationConfirmed {
        output_path: Utf8PathBuf::from("/tmp/test"),
        config: crate::command::init::config::ProjectConfig {
            organization: OrganizationId::from_str("test_org").unwrap(),
            use_case: ProjectUseCase::Connectors,
            project_name: ProjectName::from_str("my_test_project").unwrap(),
            graph_id: GraphId::from_str("my-test-graph").unwrap(),
            project_type: ProjectType::CreateNew,
        },
        composed_template,
    };

    let test_api_key = "service:my-test-graph:abc123def456"; // gitleaks:allow
    let test_graph_ref = GraphRef {
        name: "my-test-graph".to_string(),
        variant: "current".to_string(),
    };

    // Simulate the file processing logic from create_project()
    let mut processed_files = HashMap::new();
    for (file_path, content) in &mcp_creation_confirmed.composed_template.merged_files {
        let content_str = String::from_utf8_lossy(content);
        let processed_content = mcp_creation_confirmed.process_template_placeholders(
            &content_str,
            test_api_key,
            &test_graph_ref,
        );

        // Handle .env.template → .env renaming for MCP projects
        let final_path = if file_path.as_str().ends_with(".env.template") {
            Utf8PathBuf::from(file_path.as_str().replace(".env.template", ".env"))
        } else {
            file_path.clone()
        };

        processed_files.insert(final_path, processed_content);
    }

    // Verify .env.template was renamed to .env
    assert!(processed_files.contains_key(&Utf8PathBuf::from(".env")));
    assert!(!processed_files.contains_key(&Utf8PathBuf::from(".env.template")));

    // Verify .env content was processed
    let env_content = processed_files.get(&Utf8PathBuf::from(".env")).unwrap();
    assert!(env_content.contains(&format!("APOLLO_KEY=\"{}\"", test_api_key)));
    assert!(env_content.contains("PROJECT_NAME=\"my_test_project\""));
    assert!(!env_content.contains("{{APOLLO_KEY}}"));
    assert!(!env_content.contains("{{PROJECT_NAME}}"));

    // Verify README.md content was processed
    let readme_content = processed_files
        .get(&Utf8PathBuf::from("README.md"))
        .unwrap();
    assert!(readme_content.contains("# my_test_project"));
    assert!(readme_content.contains(&format!("graph ref: {}", test_graph_ref)));
    assert!(!readme_content.contains("{{PROJECT_NAME}}"));
    assert!(!readme_content.contains("{{APOLLO_GRAPH_REF}}"));

    println!("=== All processed files ===");
    for (path, content) in &processed_files {
        println!("File: {}", path);
        println!("Content:");
        println!("{}", content);
        println!("---");
    }
}
