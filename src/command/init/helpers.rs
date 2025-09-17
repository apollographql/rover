use crate::command::init::template_operations::PrintMode::Confirmation;
use crate::command::init::template_operations::print_grouped_files;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::{Style, hyperlink};

pub fn display_welcome_message() {
    println!();
    println!(
        "Welcome! This command helps you initialize a federated graph in your current directory."
    );
    println!();
    println!(
        "To learn more about init, run `{}` or visit {}",
        Style::Command.paint("rover init -h"),
        hyperlink("https://www.apollographql.com/docs/rover/commands/init")
    );
    println!();
}

pub(crate) fn get_command(api_key: &str, graph_ref: &str, supergraph_config: bool) -> String {
    #[cfg(target_os = "windows")]
    {
        let mut output = String::new();

        // PowerShell command
        output.push_str("PowerShell:\n");
        output.push_str(&format!(
            "$env:APOLLO_KEY = \"{}\"; $env:APOLLO_GRAPH_REF = \"{}\"; rover dev",
            api_key, graph_ref
        ));
        if supergraph_config {
            output.push_str(" --supergraph-config supergraph.yaml");
        }

        output.push_str("\n\n");

        // CMD
        output.push_str("Command Prompt:\n");
        output.push_str(&format!(
            "set APOLLO_KEY={} && set APOLLO_GRAPH_REF={} && rover dev",
            api_key, graph_ref
        ));
        if supergraph_config {
            output.push_str(" --supergraph-config supergraph.yaml");
        }

        output
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut output = String::new();
        output.push_str(&format!(
            "APOLLO_KEY={api_key} APOLLO_GRAPH_REF={graph_ref} rover dev"
        ));
        if supergraph_config {
            output.push_str(" --supergraph-config supergraph.yaml");
        }
        output
    }
}

pub fn generate_project_created_message(
    project_name: String,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: String,
    commands: Option<Vec<String>>,
    start_point_file: String,
    print_depth: Option<u8>,
) -> String {
    // Add welcome message
    println!(
        "\nAll set! Your graph '{}' has been created. Please review details below to see what was generated.\n",
        Style::File.paint(project_name)
    );

    print_grouped_files(artifacts.to_vec(), print_depth, Confirmation);

    println!();

    let mut output = String::new();
    // Add credentials section
    output.push_str(&format!(
        "{}\n",
        Style::Heading.paint("GraphOS credentials for your graph")
    ));
    output.push_str(&format!(
        "{}\n",
        Style::Success.paint(format!(
            "{}={} (Formatted graph-id@variant, references a graph in the Apollo GraphOS platform)",
            Style::GraphRef.paint("APOLLO_GRAPH_REF"),
            graph_ref
        ))
    ));
    output.push_str(&format!(
        "{}\n",
        Style::Success.paint(format!(
            "{}={} (This is your graph's API key)",
            Style::Command.paint("APOLLO_KEY"),
            api_key
        ))
    ));
    output.push('\n');

    // Add warning section
    output.push_str(&format!(
        "{}\n",
        Style::WarningHeading.paint("️▲ Before you proceed:")
    ));
    output
        .push_str("- Store your graph API key securely, you won't be able to access it again!\n\n");

    // Add next steps section
    output.push_str(&format!("{}\n", Style::Heading.paint("Next steps")));
    let dev_command = get_command(&api_key, &graph_ref.to_string(), !artifacts.is_empty());

    if let Some(commands) = commands {
        // Filter out empty commands
        let valid_commands: Vec<&str> = commands
            .iter()
            .filter(|cmd| !cmd.trim().is_empty())
            .map(|cmd| cmd.trim())
            .collect();

        if !valid_commands.is_empty() {
            if valid_commands.len() == 1 {
                output
                    .push_str("1) Start the subgraph server by running the following command:\n\n");
                output.push_str(&format!("{}\n", Style::Command.paint(valid_commands[0])));
            } else {
                output.push_str(
                    "1) Start the subgraph server by running the following commands in order:\n\n",
                );
                for cmd in valid_commands {
                    output.push_str(&format!("{}\n", Style::Command.paint(cmd)));
                }
            }
            output.push_str("\n2) In a new terminal, start a local development session:\n\n");
            output.push_str(&format!("{}\n\n", Style::Command.paint(dev_command)));
        } else {
            // If no valid commands, just show the rover dev command
            output.push_str("Start a local development session:\n\n");
            output.push_str(&format!("{}\n", Style::Command.paint(dev_command)));
        }
    } else {
        // If no commands provided, just show the rover dev command
        output.push_str("Start a local development session:\n\n");
        output.push_str(&format!("{}\n", Style::Command.paint(dev_command)));
    }

    output.push_str(&format!(
        "\nFor more information, check out '{start_point_file}'.\n\n"
    ));

    output
}

pub fn display_project_created_message(
    project_name: String,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: String,
    commands: Option<Vec<String>>,
    start_point_file: String,
    print_depth: Option<u8>,
) {
    let message = generate_project_created_message(
        project_name,
        artifacts,
        graph_ref,
        api_key,
        commands,
        start_point_file,
        print_depth,
    );
    println!("{message}");
}

pub fn display_use_template_message() {
    println!();
    println!(
        "To add a new subgraph to an existing graph, use `{}`.",
        Style::Command.paint("rover template")
    );
    println!(
        "To learn more about templates, run `{}`",
        Style::Command.paint("rover docs open template")
    );
}

/// Print categorized MCP file listing
pub fn print_mcp_file_categories(file_paths: Vec<Utf8PathBuf>) {
    // Group files by category
    let mut has_apollo_config = false;
    let mut has_mcp_files = false;
    let mut has_docs = false;
    let mut has_ide_files = false;
    let mut has_graphql_schemas = false;
    let mut has_tools = false;

    // Check what categories we have
    for file_path in &file_paths {
        if file_path.starts_with(".apollo/") || file_path.as_str() == "supergraph.yaml" {
            has_apollo_config = true;
        } else if file_path.as_str() == "mcp.Dockerfile" {
            has_mcp_files = true;
        } else if file_path.starts_with("tools/") {
            has_tools = true;
        } else if file_path.starts_with("docs/")
            || file_path.as_str() == "README.md"
            || file_path.as_str() == "QUICKSTART_MCP.md"
        {
            has_docs = true;
        } else if file_path.starts_with(".idea/")
            || file_path.starts_with(".vscode/")
            || file_path.as_str() == ".gitignore"
        {
            has_ide_files = true;
        } else if file_path.ends_with(".graphql") && !file_path.starts_with("tools/") {
            has_graphql_schemas = true;
        }
    }

    // Apollo Configuration
    if has_apollo_config {
        println!();
        println!("{}", Style::Prompt.paint("Apollo configuration"));
        println!("Connect to Apollo GraphOS for schema management");
        if file_paths.iter().any(|k| k.starts_with(".apollo/")) {
            println!(" .apollo/");
        }
        if file_paths.iter().any(|k| k.as_str() == "supergraph.yaml") {
            println!(" supergraph.yaml");
        }
    }

    // MCP Server Files
    if has_mcp_files || has_tools {
        println!();
        println!("{}", Style::Prompt.paint("MCP Server"));
        println!("Docker container and tools for AI interaction");
        if file_paths.iter().any(|k| k.as_str() == "mcp.Dockerfile") {
            println!(" mcp.Dockerfile");
        }
        if has_tools {
            println!(" tools/ (AI-callable operations)");
        }
    }

    // GraphQL Schemas
    if has_graphql_schemas {
        println!();
        println!("{}", Style::Prompt.paint("GraphQL Schemas"));
        println!("Your data models and API definitions");
        for file_path in file_paths.iter().filter(|k| k.ends_with(".graphql") && !k.starts_with("tools/")) {
            println!(" {}", file_path);
        }
    }

    // Documentation
    if has_docs {
        println!();
        println!("{}", Style::Prompt.paint("Documentation"));
        println!("Getting started guides and references");
        if file_paths.iter().any(|k| k.as_str() == "README.md") {
            println!(" README.md");
        }
        if file_paths.iter().any(|k| k.as_str() == "QUICKSTART_MCP.md") {
            println!(" QUICKSTART_MCP.md");
        }
        if file_paths.iter().any(|k| k.starts_with("docs/")) {
            println!(" docs/");
        }
    }

    // Development Environment
    if has_ide_files {
        println!();
        println!("{}", Style::Prompt.paint("Development environment"));
        println!("IDE configuration and project settings");
        if file_paths.iter().any(|k| k.starts_with(".vscode/")) {
            println!(" .vscode/");
        }
        if file_paths.iter().any(|k| k.starts_with(".idea/")) {
            println!(" .idea/");
        }
        if file_paths.iter().any(|k| k.as_str() == ".gitignore") {
            println!(" .gitignore");
        }
    }
}
