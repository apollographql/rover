use crate::RoverResult;
use crate::command::init::template_operations::print_grouped_files;
use crate::command::init::{states, template_operations::PrintMode::Confirmation};
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

    let has_valid_commands = if let Some(commands) = commands {
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
            output.push_str(&format!("{}\n\n", Style::Command.paint(&dev_command)));
            true
        } else {
            false
        }
    } else {
        false
    };

    if !has_valid_commands {
        // If no commands provided or no valid commands, just show the rover dev command
        output.push_str("Start a local development session:\n\n");
        output.push_str(&format!("{}\n", Style::Command.paint(&dev_command)));
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
    // Categorize files for MCP-specific display
    let mut mcp_server_files = Vec::new();
    let mut apollo_files = Vec::new();
    let mut ide_files = Vec::new();
    let mut guide_files = Vec::new();

    for file in file_paths {
        let mut file_str = file.to_string();

        // For MCP projects, display .env.template as .env since it will be renamed during processing
        if file_str.contains(".env.template") {
            file_str = file_str.replace(".env.template", ".env");
        }

        if file_str.contains("claude_desktop_config.json")
            || file_str.contains("mcp.Dockerfile")
            || file_str.contains("mcpconfig/")
            || file_str.contains(".apollo/mcp")
        {
            mcp_server_files.push(file_str);
        } else if file_str.contains("apollo.config.yaml")
            || file_str.contains("schema.graphql")
            || file_str.contains("supergraph.yaml")
            || file_str.contains(".env")
        {
            apollo_files.push(file_str);
        } else if file_str.contains(".gitignore")
            || file_str.contains(".idea/")
            || file_str.contains(".vscode/")
            || file_str.contains("tasks.json")
        {
            ide_files.push(file_str);
        } else if file_str.contains("GETTING_STARTED.md")
            || file_str.contains("MCP_README.md")
            || file_str.contains("AGENTS.md")
        {
            guide_files.push(file_str);
        }
    }

    // Print categorized files
    if !mcp_server_files.is_empty() {
        println!();
        println!(
            "{}",
            Style::Heading.paint("MCP Server: Config, sample tools and tests")
        );
        for file in mcp_server_files {
            if file.contains("mcp.Dockerfile") {
                println!("- {} (optional Docker container)", file);
            } else {
                println!("- {}", file);
            }
        }
    }

    if !apollo_files.is_empty() {
        println!();
        println!(
            "{}",
            Style::Heading.paint("Apollo GraphOS: Apollo graph credentials and project files")
        );
        for file in apollo_files {
            println!("- {}", file);
        }
    }

    if !ide_files.is_empty() {
        println!();
        println!("{}", Style::Heading.paint("IDE: Project settings"));
        for file in ide_files {
            println!("- {}", file);
        }
    }

    if !guide_files.is_empty() {
        println!();
        println!("{}", Style::Heading.paint("Guides and references"));
        for file in guide_files {
            if file.contains("GETTING_STARTED.md") {
                println!(
                    "- {} → Working with Apollo graphs and Apollo Connectors",
                    file
                );
            } else if file.contains("MCP_README.md") {
                println!("- {} → Working with Apollo MCP Server", file);
            } else if file.contains("AGENTS.md") {
                println!("- {} → Agent rules for the robots", file);
            } else {
                println!("- {}", file);
            }
        }
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

/// Context for processing MCP template placeholders
pub struct MCPTemplateContext<'a> {
    pub project_name: &'a str,
    pub graph_id: &'a str,
    pub graph_name: &'a str,
    pub variant_name: &'a str,
    pub organization_name: &'a str,
    pub api_key: &'a str,
    pub graph_ref: &'a GraphRef,
    pub mcp_server_binary: Option<&'a str>,
    pub mcp_config_path: Option<&'a str>,
    pub tools_path: Option<&'a str>,
}

/// Unified template placeholder processing for MCP projects
/// Handles both ${} and {{}} placeholder formats comprehensively
/// This ensures consistency between new project and existing graph flows
pub fn process_mcp_template_placeholders(content: &str, ctx: &MCPTemplateContext) -> String {
    let graph_ref_str = ctx.graph_ref.to_string();
    let docker_tag = normalize_docker_tag(ctx.graph_id);

    let mut processed_content = content.to_string();

    // Process both ${} format (for YAML files) and {{}} format (for other templates)
    processed_content = processed_content
        // ${} format - primarily for YAML files
        .replace("${PROJECT_NAME}", ctx.project_name)
        .replace("${DOCKER_TAG}", &docker_tag)
        .replace("${GRAPH_REF}", &graph_ref_str)
        .replace("${GRAPH_ID}", ctx.graph_id)
        .replace("${GRAPH_NAME}", ctx.graph_name)
        .replace("${VARIANT_NAME}", ctx.variant_name)
        .replace("${ORGANIZATION_NAME}", ctx.organization_name)
        .replace("${APOLLO_KEY}", ctx.api_key)
        .replace("${APOLLO_API_KEY}", ctx.api_key)
        .replace("${APOLLO_GRAPH_REF}", &graph_ref_str)
        .replace("${GRAPHQL_ENDPOINT}", "http://localhost:4000/graphql")
        .replace("${LOCAL_GRAPHQL_ENDPOINT}", "http://localhost:4000/graphql")
        .replace("${STAGING_GRAPHQL_ENDPOINT}", "http://localhost:4000")
        .replace("${PROJECT_VERSION}", "1.0.0")
        .replace(
            "${PROJECT_REPOSITORY_URL}",
            &format!("https://github.com/user/{}", ctx.project_name),
        )
        .replace(
            "${GRAPH_STUDIO_URL}",
            &format!(
                "https://studio.apollographql.com/graph/{}/explorer",
                ctx.graph_id
            ),
        )
        // {{}} format - for non-YAML templates and backwards compatibility
        .replace("{{PROJECT_NAME}}", ctx.project_name)
        .replace("{{DOCKER_TAG}}", &docker_tag)
        .replace("{{GRAPH_REF}}", &graph_ref_str)
        .replace("{{GRAPH_ID}}", ctx.graph_id)
        .replace("{{GRAPH_NAME}}", ctx.graph_name)
        .replace("{{VARIANT_NAME}}", ctx.variant_name)
        .replace("{{ORGANIZATION_NAME}}", ctx.organization_name)
        .replace("{{APOLLO_KEY}}", ctx.api_key)
        .replace("{{APOLLO_API_KEY}}", ctx.api_key)
        .replace("{{APOLLO_GRAPH_REF}}", &graph_ref_str)
        .replace(
            "{{GRAPHQL_ENDPOINT}}",
            "http://host.docker.internal:4000/graphql",
        )
        .replace(
            "{{LOCAL_GRAPHQL_ENDPOINT}}",
            "http://localhost:4000/graphql",
        )
        .replace("{{STAGING_GRAPHQL_ENDPOINT}}", "http://localhost:4000")
        .replace("{{PROJECT_VERSION}}", "1.0.0")
        .replace(
            "{{PROJECT_REPOSITORY_URL}}",
            &format!("https://github.com/user/{}", ctx.project_name),
        )
        .replace(
            "{{GRAPH_STUDIO_URL}}",
            &format!(
                "https://studio.apollographql.com/graph/{}/explorer",
                ctx.graph_id
            ),
        )
        // Quoted versions for JSON/specific formats
        .replace("\"{{PROJECT_NAME}}\"", &format!("\"{}\"", ctx.project_name))
        .replace("\"{{APOLLO_KEY}}\"", &format!("\"{}\"", ctx.api_key))
        .replace(
            "\"{{APOLLO_GRAPH_REF}}\"",
            &format!("\"{}\"", graph_ref_str),
        )
        .replace(
            "\"{{GRAPHQL_ENDPOINT}}\"",
            "\"http://host.docker.internal:4000/graphql\"",
        );

    // Handle optional MCP-specific paths
    if let Some(binary_path) = ctx.mcp_server_binary {
        processed_content = processed_content.replace("{{MCP_SERVER_BINARY}}", binary_path);
    }
    if let Some(config_path) = ctx.mcp_config_path {
        processed_content = processed_content.replace("{{MCP_CONFIG_PATH}}", config_path);
    }
    if let Some(tools_path_str) = ctx.tools_path {
        processed_content = processed_content.replace("- /tools", &format!("- {}", tools_path_str));
    }

    // Additional endpoint replacements
    processed_content = processed_content.replace(
        "endpoint: http://host.docker.internal:4000",
        "endpoint: http://localhost:4000",
    );

    processed_content
}

/// Update template files with real values from completed project
pub fn update_template_files_with_real_values(
    completed_project: &states::ProjectCreated,
) -> RoverResult<()> {
    let output_path = completed_project.output_path.clone();

    // Check if output path exists and is a directory
    if !output_path.exists() || !output_path.is_dir() {
        return Ok(()); // Nothing to process
    }

    // Helper function to recursively process all files in a directory
    fn process_directory_recursive(
        dir_path: &camino::Utf8Path,
        completed_project: &states::ProjectCreated,
    ) -> RoverResult<()> {
        use rover_std::Fs;

        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            let utf8_path = match path.to_str() {
                Some(path_str) => camino::Utf8PathBuf::from(path_str),
                None => continue, // Skip non-UTF-8 paths
            };

            if utf8_path.is_dir() {
                // Recursively process subdirectories
                process_directory_recursive(&utf8_path, completed_project)?;
            } else if utf8_path.is_file() {
                // Process individual files
                if let Ok(current_content) = std::fs::read_to_string(&utf8_path) {
                    let updated_content = current_content
                        .replace("{{APOLLO_KEY}}", &completed_project.api_key)
                        .replace("{{APOLLO_API_KEY}}", &completed_project.api_key)
                        .replace(
                            "{{APOLLO_GRAPH_REF}}",
                            &completed_project.graph_ref.to_string(),
                        )
                        .replace("{{GRAPH_REF}}", &completed_project.graph_ref.to_string())
                        .replace(
                            "{{PROJECT_NAME}}",
                            &completed_project.config.project_name.to_string(),
                        )
                        .replace("${APOLLO_KEY}", &completed_project.api_key)
                        .replace("${APOLLO_API_KEY}", &completed_project.api_key)
                        .replace(
                            "${APOLLO_GRAPH_REF}",
                            &completed_project.graph_ref.to_string(),
                        )
                        .replace("${GRAPH_REF}}", &completed_project.graph_ref.to_string())
                        .replace(
                            "${PROJECT_NAME}",
                            &completed_project.config.project_name.to_string(),
                        );

                    // Only write if content changed
                    if updated_content != current_content {
                        Fs::write_file(&utf8_path, updated_content)?;
                    }
                }
            }
        }

        Ok(())
    }

    // Start recursive processing from the output directory
    process_directory_recursive(&output_path, completed_project)?;

    Ok(())
}
