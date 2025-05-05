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
fn test_display_project_created_message_with_command() {
    let project_name = "my-graph";
    let artifacts = vec![
        Utf8PathBuf::from("supergraph.yaml"),
        Utf8PathBuf::from("getting-started.md"),
    ];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key";
    let command = Some("npm start");

    let output =
        generate_project_created_message(project_name, &artifacts, &graph_ref, api_key, command);
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        project_name
    )));
    assert!(plain_output.contains("supergraph.yaml"));
    assert!(plain_output.contains("getting-started.md"));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", api_key)));
    assert!(plain_output.contains("Store your graph API key securely"));
    assert!(plain_output.contains("npm start"));
    assert!(plain_output.contains("rover dev"));
}

#[test]
fn test_display_project_created_message_without_command() {
    let project_name = "my-graph";
    let artifacts = vec![Utf8PathBuf::from("supergraph.yaml")];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key";
    let command = None;

    let output =
        generate_project_created_message(project_name, &artifacts, &graph_ref, api_key, command);
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        project_name
    )));
    assert!(plain_output.contains("supergraph.yaml"));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", api_key)));
    assert!(plain_output.contains("Store your graph API key securely"));
    assert!(plain_output.contains("rover dev"));
    // Should not contain any command-specific text
    assert!(!plain_output.contains("Start the service"));
}

#[test]
fn test_display_project_created_message_with_empty_artifacts() {
    let project_name = "my-graph";
    let artifacts: Vec<Utf8PathBuf> = vec![];
    let graph_ref = GraphRef::new("my-graph".to_string(), Some("main".to_string())).unwrap();
    let api_key = "test-api-key";
    let command = None;

    let output =
        generate_project_created_message(project_name, &artifacts, &graph_ref, api_key, command);
    let plain_output = strip_ansi_codes(&output);

    // Test that the output contains expected content
    assert!(plain_output.contains(&format!(
        "All set! Your graph '{}' has been created",
        project_name
    )));
    assert!(plain_output.contains(&format!("APOLLO_GRAPH_REF={}", graph_ref)));
    assert!(plain_output.contains(&format!("APOLLO_KEY={}", api_key)));
    // Should not contain the files section
    assert!(!plain_output.contains("Files created:"));
    // Should not contain supergraph.yaml in the files section
    assert!(!plain_output.contains("supergraph.yaml\n"));
}
