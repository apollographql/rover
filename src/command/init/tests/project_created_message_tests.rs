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
        Utf8PathBuf::from("rover.yaml"),
        Utf8PathBuf::from("getting-started.md"),
    ];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key".to_string();
    let commands = &["npm ci"];
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
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={graph_ref}")));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    assert!(plain_output.contains("Store your graph API key securely"));
    assert!(plain_output.contains("1) Start the subgraph server by running the following command:"));
    assert!(plain_output.contains("npm ci"));
    assert!(plain_output.contains("2) In a new terminal, start a local development session:"));
    assert!(plain_output.contains("rover dev"));
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "getting-started.md"
    )));
}

#[test]
fn test_display_project_created_message_with_multiple_commands() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("rover.yaml")];
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
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output
        .contains("1) Start the subgraph server by running the following commands in order:"));
    assert!(plain_output.contains("npm install"));
    assert!(plain_output.contains("npm run build"));
    assert!(plain_output.contains("npm start"));
    assert!(plain_output.contains("2) In a new terminal, start a local development session:"));
    assert!(plain_output.contains("rover dev"));
}

#[test]
fn test_display_project_created_message_with_empty_command_array() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("rover.yaml")];
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
        None,
    );
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains("Start a local development session:"));
    assert!(plain_output.contains("rover dev"));
    // Should not contain any command-specific text
    assert!(!plain_output.contains("Start the subgraph server"));
}

#[test]
fn test_display_project_created_message_without_command() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("rover.yaml")];
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
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={graph_ref}")));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    assert!(plain_output.contains("Store your graph API key securely"));
    assert!(plain_output.contains("Start a local development session:"));
    assert!(plain_output.contains("rover dev"));
    // Should not contain any command-specific text
    assert!(!plain_output.contains("Start the subgraph server"));
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "getting-started.md"
    )));
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
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={graph_ref}")));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    // Should not contain the files section
    assert!(!plain_output.contains("Files created:"));
    // Should not contain rover.yaml in the files section
    assert!(!plain_output.contains("rover.yaml\n"));
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "getting-started.md"
    )));
}

#[test]
fn test_display_project_created_message_with_custom_start_point() {
    let project_name = "my-graph".to_string();
    let artifacts = vec![Utf8PathBuf::from("rover.yaml")];
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
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={graph_ref}")));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", "test-api-key")));
    // Should contain the custom start point file
    assert!(plain_output.contains(&format!(
        "For more information, check out '{}'",
        "readme.md"
    )));
    // Should not contain the default start point file
    assert!(!plain_output.contains("For more information, check out 'getting-started.md'"));
}
