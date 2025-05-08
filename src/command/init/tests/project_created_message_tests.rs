use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use std::string::String;

use crate::command::init::helpers::generate_project_created_message;

// Helper function to strip ANSI color codes
fn strip_ansi_codes(s: &str) -> String {
    s.replace("\x1b[0m", "")
        .replace("\x1b[1m", "")
        .replace("\x1b[32m", "")
        .replace("\x1b[33m", "")
        .replace("\x1b[34m", "")
        .replace("\x1b[35m", "")
        .replace("\x1b[36m", "")
        .replace("\x1b[37m", "")
        .replace("\x1b[38;5;", "")
        .replace("\x1b[39m", "")
}

#[test]
fn test_display_project_created_message_with_single_command() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![
        Utf8PathBuf::from("supergraph.yaml"),
        Utf8PathBuf::from("getting-started.md"),
    ];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = &["npm ci && npm start"];
    let commands = Some(commands.iter().map(|&s| s.to_string()).collect::<Vec<_>>());
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        "my-graph"
    )));

    // Files section
    assert!(plain_output.contains("Files created:"));
    assert!(plain_output.contains("supergraph.yaml"));
    assert!(plain_output.contains("getting-started.md"));

    // Credentials section
    assert!(plain_output.contains("GraphOS credentials for your graph"));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));

    // Warning section
    assert!(plain_output.contains("▲ Before you proceed:"));
    assert!(plain_output.contains("Store your graph API key securely"));

    // Next steps section
    assert!(plain_output.contains("Next steps"));
    assert!(plain_output.contains("1) Run: npm ci && npm start"));
    assert!(plain_output.contains("2) Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Documentation reference
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "getting-started.md"
    )));

    // Verify no unexpected command prefixes
    assert!(!plain_output.contains("3) Run:"));
    assert!(!plain_output.contains("4) Run:"));
}

#[test]
fn test_display_project_created_message_with_multiple_commands() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = &["npm install", "npm run build", "npm start"];
    let commands = Some(commands.iter().map(|&s| s.to_string()).collect::<Vec<_>>());
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains("Next steps"));
    assert!(plain_output.contains("1) Run: npm install"));
    assert!(plain_output.contains("2) Run: npm run build"));
    assert!(plain_output.contains("3) Run: npm start"));
    assert!(plain_output.contains("4) Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Verify no unexpected command prefixes
    assert!(!plain_output.contains("5) Run:"));
    assert!(!plain_output.contains("0) Run:"));
}

#[test]
fn test_display_project_created_message_with_empty_command_array() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = Some(Vec::new());
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
    );
    let plain_output = strip_ansi_codes(&output);
    
    // Test that the output contains expected content
    assert!(plain_output.contains("Next steps"));
    assert!(plain_output.contains("Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Verify no command prefixes are present
    assert!(!plain_output.contains("Run:"));
    assert!(!plain_output.contains("npm"));
    assert!(!plain_output.contains("2)"));
}

#[test]
fn test_display_project_created_message_without_command() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = None;
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        "my-graph"
    )));
    assert!(plain_output.contains("supergraph.yaml"));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    assert!(plain_output.contains("Store your graph API key securely"));
    assert!(plain_output.contains("Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Verify no command prefixes or numbered steps are present
    assert!(!plain_output.contains("Run:"));
    assert!(!plain_output.contains("1)"));
    assert!(!plain_output.contains("2)"));
    assert!(!plain_output.contains("npm"));
}

#[test]
fn test_display_project_created_message_with_empty_artifacts() {
    let project_name = "my-graph".to_string();
    let artifacts: Vec<Utf8PathBuf> = vec![];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = None;
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        "my-graph"
    )));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    assert!(plain_output.contains("Start a local development session"));
    assert!(plain_output.contains("rover dev"));
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "getting-started.md"
    )));

    // Verify files section is not present
    assert!(!plain_output.contains("Files created:"));
    assert!(!plain_output.contains("supergraph.yaml"));
    // Only check for supergraph.yaml in the files section
    assert!(!plain_output.contains("supergraph.yaml\n"));
}

#[test]
fn test_display_project_created_message_with_custom_start_point() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = None;
    let start_point_file = "readme.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        "my-graph"
    )));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    assert!(plain_output.contains("Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Verify custom start point file is referenced
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "readme.md"
    )));

    // Verify default start point file is not referenced
    assert!(!plain_output.contains("For more information, check out 'getting-started.md'"));
}

#[test]
fn test_display_project_created_message_with_empty_string_commands() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = Some(vec![
        "npm install".to_string(),
        "".to_string(),
        "   ".to_string(),
        "npm start".to_string(),
    ]);
    let start_point_file = "getting-started.md".to_string();

    let output = generate_project_created_message(
        project_name,
        &artifacts,
        &graph_ref,
        api_key,
        commands,
        start_point_file,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains("Next steps"));
    assert!(plain_output.contains("1) Run: npm install"));
    assert!(plain_output.contains("2) Run: npm start"));
    assert!(plain_output.contains("3) Start a local development session"));
    assert!(plain_output.contains("rover dev"));

    // Verify empty commands are not displayed
    assert!(!plain_output.contains("Run:   "));  // Whitespace-only command
    assert!(!plain_output.contains("Run: \n"));  // Empty command
    assert!(!plain_output.contains("4)"));       // No extra command numbers
}
