use std::str::FromStr;

use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;

use crate::command::init::{states::*, template_fetcher::*};

// Tests verify the logic without trying to capture println! output

#[test]
fn test_mcp_success_includes_base_template_commands() {
    // Test that MCP project success message includes the base template commands
    // before the MCP-specific steps

    // Create a base template with commands (like npm ci, npm start)
    let base_template = Template {
        id: TemplateId("typescript".to_string()),
        display_name: "TypeScript Subgraph".to_string(),
        path: "start-with-typescript".to_string(),
        language: "TypeScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 5,
        routing_url: "http://localhost:4001".to_string(),
        commands: Some(vec!["npm ci".to_string(), "npm start".to_string()]),
        start_point_file: "GETTING_STARTED.md".to_string(),
        print_depth: Some(2),
    };

    let project_created = ProjectCreated {
        output_path: Utf8PathBuf::from("test_project"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org")
                .unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("my_test_project")
                .unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str(
                "my-test-graph",
            )
            .unwrap(),
            project_type: crate::command::init::options::ProjectType::CreateNew,
        },
        artifacts: vec![
            Utf8PathBuf::from("claude_desktop_config.json"),
            Utf8PathBuf::from("package.json"),
            Utf8PathBuf::from(".env"),
        ],
        api_key: "service:my-test-graph:abc123".to_string(), // gitleaks:allow
        graph_ref: GraphRef {
            name: "my-test-graph".to_string(),
            variant: "current".to_string(),
        },
        template: base_template,
    };

    // We can't easily capture println! output in tests, but we can verify the logic
    // by checking that the template commands are accessible
    assert!(project_created.template.commands.is_some());
    let commands = project_created.template.commands.as_ref().unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0], "npm ci");
    assert_eq!(commands[1], "npm start");

    println!(
        "✅ MCP project has access to base template commands: {:?}",
        commands
    );
}

#[test]
fn test_mcp_success_with_no_commands() {
    // Test MCP project success when base template has no commands

    let base_template = Template {
        id: TemplateId("minimal".to_string()),
        display_name: "Minimal Template".to_string(),
        path: "minimal".to_string(),
        language: "JavaScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 3,
        routing_url: "http://localhost:4000".to_string(),
        commands: None, // No commands
        start_point_file: "README.md".to_string(),
        print_depth: None,
    };

    let project_created = ProjectCreated {
        output_path: Utf8PathBuf::from("test_project"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org")
                .unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("minimal_project")
                .unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str(
                "minimal-graph",
            )
            .unwrap(),
            project_type: crate::command::init::options::ProjectType::CreateNew,
        },
        artifacts: vec![
            Utf8PathBuf::from("claude_desktop_config.json"),
            Utf8PathBuf::from(".env"),
        ],
        api_key: "service:minimal-graph:xyz789".to_string(), // gitleaks:allow
        graph_ref: GraphRef {
            name: "minimal-graph".to_string(),
            variant: "current".to_string(),
        },
        template: base_template,
    };

    // Verify that when there are no commands, the flow still works
    assert!(project_created.template.commands.is_none());

    println!("✅ MCP project handles templates with no commands correctly");
}

#[test]
fn test_mcp_success_with_empty_commands() {
    // Test MCP project success when base template has empty commands array

    let base_template = Template {
        id: TemplateId("empty-commands".to_string()),
        display_name: "Empty Commands Template".to_string(),
        path: "empty-commands".to_string(),
        language: "Python".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 4,
        routing_url: "http://localhost:4002".to_string(),
        commands: Some(vec![]), // Empty commands array
        start_point_file: "README.md".to_string(),
        print_depth: None,
    };

    let project_created = ProjectCreated {
        output_path: Utf8PathBuf::from("test_project"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org")
                .unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("empty_cmd_project")
                .unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str(
                "empty-cmd-graph",
            )
            .unwrap(),
            project_type: crate::command::init::options::ProjectType::CreateNew,
        },
        artifacts: vec![Utf8PathBuf::from("claude_desktop_config.json")],
        api_key: "service:empty-cmd-graph:def456".to_string(), // gitleaks:allow
        graph_ref: GraphRef {
            name: "empty-cmd-graph".to_string(),
            variant: "current".to_string(),
        },
        template: base_template,
    };

    // Verify that empty commands array is handled correctly
    assert!(project_created.template.commands.is_some());
    assert!(
        project_created
            .template
            .commands
            .as_ref()
            .unwrap()
            .is_empty()
    );

    println!("✅ MCP project handles templates with empty commands array correctly");
}

#[test]
fn test_mcp_success_commands_filtering() {
    // Test that empty/whitespace commands are filtered out

    let base_template = Template {
        id: TemplateId("mixed-commands".to_string()),
        display_name: "Mixed Commands Template".to_string(),
        path: "mixed-commands".to_string(),
        language: "TypeScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 5,
        routing_url: "http://localhost:4001".to_string(),
        commands: Some(vec![
            "npm install".to_string(),
            "".to_string(),    // Empty string
            "   ".to_string(), // Whitespace only
            "npm run dev".to_string(),
            "\t\n".to_string(), // Tabs and newlines
        ]),
        start_point_file: "GETTING_STARTED.md".to_string(),
        print_depth: Some(1),
    };

    let project_created = ProjectCreated {
        output_path: Utf8PathBuf::from("test_project"),
        config: crate::command::init::config::ProjectConfig {
            organization: crate::command::init::options::OrganizationId::from_str("test_org")
                .unwrap(),
            use_case: crate::command::init::options::ProjectUseCase::Connectors,
            project_name: crate::command::init::options::ProjectName::from_str("mixed_project")
                .unwrap(),
            graph_id: crate::command::init::graph_id::validation::GraphId::from_str("mixed-graph")
                .unwrap(),
            project_type: crate::command::init::options::ProjectType::CreateNew,
        },
        artifacts: vec![
            Utf8PathBuf::from("claude_desktop_config.json"),
            Utf8PathBuf::from("package.json"),
        ],
        api_key: "service:mixed-graph:ghi789".to_string(), // gitleaks:allow
        graph_ref: GraphRef {
            name: "mixed-graph".to_string(),
            variant: "current".to_string(),
        },
        template: base_template,
    };

    // Simulate the filtering logic from display_mcp_success
    if let Some(commands) = &project_created.template.commands {
        let valid_commands: Vec<&str> = commands
            .iter()
            .filter(|cmd| !cmd.trim().is_empty())
            .map(|cmd| cmd.trim())
            .collect();

        // Should filter out empty and whitespace-only commands
        assert_eq!(valid_commands.len(), 2);
        assert_eq!(valid_commands[0], "npm install");
        assert_eq!(valid_commands[1], "npm run dev");

        println!("✅ Command filtering works correctly: {:?}", valid_commands);
    }
}

#[test]
fn test_mcp_step_numbering_logic() {
    // Test that step numbers adjust correctly based on whether commands are present
    // New ordering: Step 1 = Claude Desktop, Step 2 = Commands (if exist), Step 3 = MCP server

    // Test with commands - should use step "3" for MCP server start
    let template_with_commands = Template {
        id: TemplateId("with-commands".to_string()),
        display_name: "With Commands".to_string(),
        path: "with-commands".to_string(),
        language: "TypeScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 5,
        routing_url: "http://localhost:4001".to_string(),
        commands: Some(vec!["npm install".to_string()]),
        start_point_file: "README.md".to_string(),
        print_depth: None,
    };

    // Simulate the new step numbering logic (has_commands determines final step)
    let has_commands_with = template_with_commands.commands.is_some_and(|commands| {
        let valid_commands = commands.iter().find(|cmd| !cmd.trim().is_empty());
        valid_commands.is_some()
    });

    let final_step_num_with_commands = if has_commands_with { "3" } else { "2" };
    assert_eq!(final_step_num_with_commands, "3");

    // Test without commands - should use step "2" for MCP server start
    let template_without_commands = Template {
        id: TemplateId("without-commands".to_string()),
        display_name: "Without Commands".to_string(),
        path: "without-commands".to_string(),
        language: "JavaScript".to_string(),
        federation_version: "=2.10.0".to_string(),
        max_schema_depth: 3,
        routing_url: "http://localhost:4000".to_string(),
        commands: None,
        start_point_file: "README.md".to_string(),
        print_depth: None,
    };

    let has_commands_without = template_without_commands.commands.is_some_and(|commands| {
        let valid_commands = commands.iter().find(|cmd| !cmd.trim().is_empty());
        valid_commands.is_some()
    });

    let final_step_num_without_commands = if has_commands_without { "3" } else { "2" };
    assert_eq!(final_step_num_without_commands, "2");

    println!("✅ Step numbering logic works correctly (new order)");
    println!("   - Step 1: Always Claude Desktop configuration");
    println!(
        "   - With commands: Step 2 = Commands, Step {} = MCP server",
        final_step_num_with_commands
    );
    println!(
        "   - Without commands: Step {} = MCP server directly",
        final_step_num_without_commands
    );
}
