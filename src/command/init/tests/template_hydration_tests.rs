#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashMap, str::FromStr};

#[cfg(unix)]
use camino::Utf8Path;
use camino::Utf8PathBuf;
use rover_studio::types::GraphRef;
use rstest::{fixture, rstest};
use speculoos::prelude::*;

#[cfg(unix)]
use crate::command::init::helpers::write_project_file;
use crate::command::init::{
    graph_id::validation::GraphId,
    helpers::{
        ExistingGraphMCPContext, env_output_path, hydrate_env_api_key, hydrate_existing_graph_file,
        is_env_file,
    },
    options::*,
    states::*,
    template_fetcher::{Template, TemplateId},
};

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
    let test_graph_ref = GraphRef::new("my-test-graph", Some("current")).unwrap();

    // Process .env.template content
    let env_content = r#"APOLLO_KEY={{APOLLO_KEY}}
APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}
PROJECT_NAME={{PROJECT_NAME}}
GRAPHQL_ENDPOINT={{GRAPHQL_ENDPOINT}}
"#;

    let processed_env =
        mcp_creation_confirmed.process_template_placeholders(env_content, &test_graph_ref);

    println!("=== Original .env.template content ===");
    println!("{}", env_content);
    println!("=== Processed .env content ===");
    println!("{}", processed_env);

    // Verify the placeholders were replaced, except the API key, which only `.env` receives
    assert!(processed_env.contains("APOLLO_KEY={{APOLLO_KEY}}"));
    assert!(processed_env.contains(&format!("APOLLO_GRAPH_REF={}", test_graph_ref)));
    assert!(processed_env.contains("PROJECT_NAME=my_test_project"));
    assert!(processed_env.contains("GRAPHQL_ENDPOINT=http://host.docker.internal:4000/graphql"));

    // Verify placeholders are no longer present
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

    let processed_yaml =
        mcp_creation_confirmed.process_template_placeholders(yaml_content, &test_graph_ref);

    println!("=== Original YAML content ===");
    println!("{}", yaml_content);
    println!("=== Processed YAML content ===");
    println!("{}", processed_yaml);

    // Verify ${} placeholders were replaced
    assert!(processed_yaml.contains("project: my_test_project"));
    assert!(processed_yaml.contains("apollo_key: ${APOLLO_KEY}"));
    assert!(processed_yaml.contains(&format!("graph_ref: {}", test_graph_ref)));
    assert!(processed_yaml.contains("endpoint: http://localhost:4000/graphql"));

    // Verify ${} placeholders are no longer present
    assert!(!processed_yaml.contains("${PROJECT_NAME}"));
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
    let test_graph_ref = GraphRef::new("my-test-graph", Some("current")).unwrap();

    // Run the file processing logic from create_project()
    let processed_files: HashMap<Utf8PathBuf, String> = mcp_creation_confirmed
        .hydrate_merged_files(test_api_key, &test_graph_ref)
        .into_iter()
        .map(|(path, content)| (path, String::from_utf8(content).unwrap()))
        .collect();

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

const CANARY_API_KEY: &str = "service:canary-graph:CANARY-KEY-0123456789"; // gitleaks:allow
const CANARY_GRAPH_REF: &str = "canary-graph@current";

/// Template files that reference the API key in every supported placeholder style.
///
/// These are synthetic files. They exercise every placeholder style, so they are not copies of
/// the real starter templates, which read the key from the environment (`${env.APOLLO_KEY}`).
#[fixture]
fn api_key_template_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            ".env.template",
            "APOLLO_KEY=\"{{APOLLO_KEY}}\"\nAPOLLO_GRAPH_REF=\"{{APOLLO_GRAPH_REF}}\"\n",
        ),
        (
            ".apollo/mcp.staging.yaml",
            "graphos:\n  apollo_graph_ref: ${APOLLO_GRAPH_REF}\nheaders:\n  Authorization: \"Bearer ${APOLLO_KEY}\"\n  X-Env-Authorization: \"Bearer ${env.APOLLO_KEY}\"\n",
        ),
        (
            ".vscode/settings.json",
            "{\n  \"APOLLO_KEY\": \"{{APOLLO_KEY}}\",\n  \"APOLLO_GRAPH_REF\": \"{{APOLLO_GRAPH_REF}}\"\n}\n",
        ),
        (
            "mcp.Dockerfile",
            "ENV APOLLO_KEY={{APOLLO_KEY}}\nENV APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}\n",
        ),
    ]
}

/// The files expected after hydrating `api_key_template_files` for `CANARY_GRAPH_REF`
#[fixture]
fn expected_hydrated_files() -> HashMap<Utf8PathBuf, String> {
    HashMap::from([
        (
            Utf8PathBuf::from(".env"),
            format!("APOLLO_KEY=\"{CANARY_API_KEY}\"\nAPOLLO_GRAPH_REF=\"{CANARY_GRAPH_REF}\"\n"),
        ),
        (
            Utf8PathBuf::from(".apollo/mcp.staging.yaml"),
            format!(
                "graphos:\n  apollo_graph_ref: {CANARY_GRAPH_REF}\nheaders:\n  Authorization: \"Bearer ${{APOLLO_KEY}}\"\n  X-Env-Authorization: \"Bearer ${{env.APOLLO_KEY}}\"\n"
            ),
        ),
        (
            Utf8PathBuf::from(".vscode/settings.json"),
            format!(
                "{{\n  \"APOLLO_KEY\": \"{{{{APOLLO_KEY}}}}\",\n  \"APOLLO_GRAPH_REF\": \"{CANARY_GRAPH_REF}\"\n}}\n"
            ),
        ),
        (
            Utf8PathBuf::from("mcp.Dockerfile"),
            format!("ENV APOLLO_KEY={{{{APOLLO_KEY}}}}\nENV APOLLO_GRAPH_REF={CANARY_GRAPH_REF}\n"),
        ),
    ])
}

/// A minimal base template for building template states in tests
#[fixture]
fn test_template() -> Template {
    Template {
        id: TemplateId("test".to_string()),
        display_name: "Test Template".to_string(),
        path: "test".to_string(),
        language: "TypeScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 5,
        routing_url: "http://localhost:4001".to_string(),
        commands: None,
        start_point_file: "README.md".to_string(),
        print_depth: None,
    }
}

fn assert_api_key_only_in_env(files: &HashMap<Utf8PathBuf, String>) {
    for (path, content) in files {
        if !is_env_file(path) {
            assert!(
                !content.contains(CANARY_API_KEY),
                "{path} should not contain the API key"
            );
        }
    }
}

#[rstest]
fn test_new_project_writes_api_key_only_to_env(
    api_key_template_files: Vec<(&'static str, &'static str)>,
    expected_hydrated_files: HashMap<Utf8PathBuf, String>,
    test_template: Template,
) {
    let merged_files = api_key_template_files
        .into_iter()
        .map(|(path, content)| (Utf8PathBuf::from(path), content.as_bytes().to_vec()))
        .collect();

    let mcp_creation_confirmed = MCPCreationConfirmed {
        output_path: Utf8PathBuf::from("/tmp/test"),
        config: crate::command::init::config::ProjectConfig {
            organization: OrganizationId::from_str("test_org").unwrap(),
            use_case: ProjectUseCase::Connectors,
            project_name: ProjectName::from_str("canary_project").unwrap(),
            graph_id: GraphId::from_str("canary-graph").unwrap(),
            project_type: ProjectType::CreateNew,
        },
        composed_template: MCPComposedTemplate::new(test_template, merged_files),
    };
    let graph_ref = GraphRef::from_str(CANARY_GRAPH_REF).unwrap();

    let hydrated: HashMap<Utf8PathBuf, String> = mcp_creation_confirmed
        .hydrate_merged_files(CANARY_API_KEY, &graph_ref)
        .into_iter()
        .map(|(path, content)| (path, String::from_utf8(content).unwrap()))
        .collect();

    assert_that!(hydrated).is_equal_to(&expected_hydrated_files);
    assert_api_key_only_in_env(&hydrated);
}

#[rstest]
fn test_existing_graph_writes_api_key_only_to_env(
    api_key_template_files: Vec<(&'static str, &'static str)>,
    expected_hydrated_files: HashMap<Utf8PathBuf, String>,
) {
    let ctx = ExistingGraphMCPContext {
        project_name: "canary-graph",
        graph_ref: CANARY_GRAPH_REF,
        graph_id: "canary-graph",
        graph_name: "Canary Graph",
        variant_name: "current",
        organization_name: "test_org",
        api_key: CANARY_API_KEY,
        graph_studio_url: "https://studio.apollographql.com/graph/canary-graph/explorer",
        mcp_server_binary: "/home/test/.rover/bin/apollo-mcp-server",
        mcp_config_path: "/project/.apollo/mcp.claude.yaml",
        tools_path: "/project/tools",
    };

    let hydrated: HashMap<Utf8PathBuf, String> = api_key_template_files
        .into_iter()
        .map(|(path, content)| hydrate_existing_graph_file(&Utf8PathBuf::from(path), content, &ctx))
        .collect();

    assert_that!(hydrated).is_equal_to(&expected_hydrated_files);
    assert_api_key_only_in_env(&hydrated);
}

#[rstest]
#[case::root(".env.template", Some(".env"))]
#[case::nested("nested/.env.template", Some("nested/.env"))]
#[case::suffix_only("local.env.template", None)]
#[case::env_example(".env.example", None)]
#[case::other("README.md", None)]
fn test_env_output_path(#[case] template_path: &str, #[case] expected: Option<&str>) {
    assert_that!(env_output_path(&Utf8PathBuf::from(template_path)))
        .is_equal_to(expected.map(Utf8PathBuf::from));
}

#[rstest]
#[case::double_brace_key("APOLLO_KEY={{APOLLO_KEY}}", "APOLLO_KEY=service:canary:key")] // gitleaks:allow
#[case::double_brace_api_key("APOLLO_KEY={{APOLLO_API_KEY}}", "APOLLO_KEY=service:canary:key")] // gitleaks:allow
#[case::dollar_key("APOLLO_KEY=${APOLLO_KEY}", "APOLLO_KEY=service:canary:key")] // gitleaks:allow
#[case::dollar_api_key("APOLLO_KEY=${APOLLO_API_KEY}", "APOLLO_KEY=service:canary:key")] // gitleaks:allow
#[case::quoted_key("APOLLO_KEY=\"{{APOLLO_KEY}}\"", "APOLLO_KEY=\"service:canary:key\"")] // gitleaks:allow
#[case::env_reference_preserved("KEY=${env.APOLLO_KEY}", "KEY=${env.APOLLO_KEY}")]
#[case::other_placeholders_preserved(
    "APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}",
    "APOLLO_GRAPH_REF={{APOLLO_GRAPH_REF}}"
)]
fn test_hydrate_env_api_key(#[case] content: &str, #[case] expected: &str) {
    assert_eq!(
        hydrate_env_api_key(content, "service:canary:key"), // gitleaks:allow
        expected
    );
}

#[cfg(unix)]
fn file_mode(path: &Utf8Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// `existing_mode` is the mode of a file already at the path, or `None` when there is no file.
#[cfg(unix)]
#[rstest]
#[case::new_env(".env", None, 0o600)]
#[case::existing_env(".env", Some(0o644), 0o600)]
#[case::existing_world_writable_env(".env", Some(0o666), 0o600)]
#[case::nested_env("nested/.env", Some(0o644), 0o600)]
#[case::config_file(".apollo/mcp.local.yaml", Some(0o644), 0o644)]
#[case::env_example(".env.example", Some(0o644), 0o644)]
fn test_write_project_file_keeps_only_env_private(
    #[case] relative: &str,
    #[case] existing_mode: Option<u32>,
    #[case] expected_mode: u32,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join(relative)).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if let Some(mode) = existing_mode {
        // Start from a known mode so the result does not depend on the process umask
        std::fs::write(&path, "OLD=value\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    write_project_file(&path, "APOLLO_KEY=placeholder\n").unwrap();

    assert_that!(file_mode(&path)).is_equal_to(expected_mode);
    assert_that!(std::fs::read_to_string(&path).unwrap())
        .is_equal_to("APOLLO_KEY=placeholder\n".to_string());
}

#[cfg(unix)]
#[rstest]
fn test_write_template_creates_env_private_to_its_owner(test_template: Template) {
    let dir = tempfile::tempdir().unwrap();
    let output_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let selected_template = SelectedTemplateState {
        template: test_template,
        files: HashMap::from([
            (
                Utf8PathBuf::from(".env"),
                b"APOLLO_KEY=placeholder\n".to_vec(),
            ),
            (
                Utf8PathBuf::from(".apollo/mcp.local.yaml"),
                b"endpoint: http://localhost:4000\n".to_vec(),
            ),
        ]),
    };

    selected_template.write_template(&output_path).unwrap();

    assert_that!(file_mode(&output_path.join(".env"))).is_equal_to(0o600);
    assert_that!(std::fs::read_to_string(output_path.join(".env")).unwrap())
        .is_equal_to("APOLLO_KEY=placeholder\n".to_string());
    assert_that!(std::fs::read_to_string(output_path.join(".apollo/mcp.local.yaml")).unwrap())
        .is_equal_to("endpoint: http://localhost:4000\n".to_string());
}
