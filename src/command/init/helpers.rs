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

pub(crate) fn get_command(supergraph_config: bool) -> String {
    let mut output = String::from("rover dev");
    if supergraph_config {
        output.push_str(" --supergraph-config supergraph.yaml");
    }
    output
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
    let dev_command = get_command(!artifacts.is_empty());

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
    // MCP Server: Config, sample tools and tests
    println!();
    println!(
        "{}",
        Style::Prompt.paint("MCP Server: Config, sample tools and tests")
    );
    if file_paths
        .iter()
        .any(|k| k.as_str() == "claude_desktop_config.json")
    {
        println!("- claude_desktop_config.json");
    }
    if file_paths.iter().any(|k| k.as_str() == "mcp.Dockerfile") {
        println!("- mcp.Dockerfile (optional Docker container)");
    }
    if file_paths
        .iter()
        .any(|k| k.as_str() == "mcpconfig/mcp.local.yaml")
    {
        println!("- mcpconfig/mcp.local.yaml");
    }
    if file_paths
        .iter()
        .any(|k| k.as_str() == "mcpconfig/mcp.staging.yaml")
    {
        println!("- mcpconfig/mcp.staging.yaml");
    }

    // Apollo GraphOS: Apollo graph credentials and project files
    println!();
    println!(
        "{}",
        Style::Prompt.paint("Apollo GraphOS: Apollo graph credentials and project files")
    );
    if file_paths
        .iter()
        .any(|k| k.as_str() == "apollo.config.yaml")
    {
        println!("- apollo.config.yaml");
    }
    if file_paths.iter().any(|k| k.as_str() == "schema.graphql") {
        println!("- schema.graphql");
    }
    if file_paths.iter().any(|k| k.as_str() == "supergraph.yaml") {
        println!("- supergraph.yaml");
    }
    if file_paths.iter().any(|k| k.as_str() == ".env") {
        println!("- .env");
    }

    // IDE: Project settings
    println!();
    println!("{}", Style::Prompt.paint("IDE: Project settings"));
    if file_paths.iter().any(|k| k.as_str() == ".gitignore") {
        println!("- .gitignore");
    }
    if file_paths.iter().any(|k| k.as_str() == ".env") {
        println!("- .env");
    }
    if file_paths.iter().any(|k| k.starts_with(".idea/")) {
        println!("- .idea/");
    }
    if file_paths.iter().any(|k| k.starts_with(".vscode/")) {
        println!("- .vscode/");
    }
    if file_paths.iter().any(|k| k.as_str() == "tasks.json") {
        println!("- tasks.json");
    }

    // Guides and references
    println!();
    println!("{}", Style::Prompt.paint("Guides and references"));
    if file_paths
        .iter()
        .any(|k| k.as_str() == "GETTING_STARTED.MD")
    {
        println!("- GETTING_STARTED.MD → Working with Apollo graphs and Apollo Connectors");
    }
    if file_paths.iter().any(|k| k.as_str() == "MCP_README.md") {
        println!("- MCP_README.md → Working with Apollo MCP Server");
    }
    if file_paths.iter().any(|k| k.as_str() == "AGENTS.md") {
        println!("- AGENTS.md → Agent rules for the robots");
    }
}

/// Normalizes a graph ID into a valid Docker image tag name
/// Docker image tags must be lowercase and cannot contain spaces or special characters
pub fn normalize_docker_tag(graph_id: &str) -> String {
    graph_id
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}
