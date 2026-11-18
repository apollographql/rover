use camino::{Utf8Path, Utf8PathBuf};
use indoc::formatdoc;
use rover_std::{Style, hyperlink};
use rover_studio::types::GraphRef;

use crate::{
    RoverResult,
    command::init::{
        GraphVariantOption, states,
        template_operations::{PrintMode::Confirmation, print_grouped_files},
    },
};

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

/// Formats an API key for display after it has been written to `.env`:
/// the key is masked and followed by a pointer to the file that holds it.
pub(crate) fn api_key_saved_line(api_key: &str) -> String {
    format!("{} (saved to .env)", houston::mask_key(api_key))
}

/// Builds the "Selected Graph" summary shown after `rover init --mcp`
/// adds an MCP server to a project for an existing graph.
pub(crate) fn existing_graph_mcp_summary(
    graph_name: &str,
    variant_name: &str,
    organization_name: &str,
    graph_ref: &str,
    supergraph_downloaded: bool,
    api_key: &str,
) -> String {
    let mut output = format!("\n{}\n", Style::Heading.paint("Selected Graph:"));
    output.push_str(&format!("  • Graph: {} ({})\n", graph_name, variant_name));
    output.push_str(&format!("  • Organization: {}\n", organization_name));
    output.push_str(&format!("  • Graph Reference: {}\n", graph_ref));
    if supergraph_downloaded {
        output.push_str("  • Supergraph schema: Downloaded\n");
    }
    output.push_str(&format!(
        "  • Service API key: {}\n",
        api_key_saved_line(api_key)
    ));
    output
}

/// Contents of a minimal `.env` holding the credentials `rover init --mcp`
/// points users to.
pub(crate) fn env_file_contents(api_key: &str, graph_ref: &str) -> String {
    format!("APOLLO_KEY={api_key}\nAPOLLO_GRAPH_REF={graph_ref}\n")
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

/// Port the generated MCP server listens on.
///
/// Matches `transport.port` in the rover-init-starters `add-mcp` MCP config.
pub(crate) const MCP_DEFAULT_PORT: u16 = 8000;

/// Local MCP endpoint URL for a server started from the generated config.
pub(crate) fn mcp_local_endpoint() -> String {
    format!("http://127.0.0.1:{MCP_DEFAULT_PORT}/mcp")
}

/// `docker run` command for the MCP image built as `{docker_tag}-mcp`.
pub(crate) fn mcp_docker_run_hint(docker_tag: &str) -> String {
    format!(
        "docker run -p 127.0.0.1:{MCP_DEFAULT_PORT}:{MCP_DEFAULT_PORT} --env-file .env {docker_tag}-mcp"
    )
}

/// README written into an existing graph's directory by `rover init --mcp`.
pub(super) fn existing_graph_mcp_readme(
    selected_graph: &GraphVariantOption,
    graph_ref_str: &str,
    subgraph_info: &str,
    docker_tag: &str,
    graph_endpoint: &str,
) -> String {
    let subgraph_line = if subgraph_info.is_empty() {
        String::new()
    } else {
        format!("\n- **{subgraph_info}**")
    };

    formatdoc!(
        r#"
        # {graph_name} MCP Server

        This MCP server provides AI-accessible tools for your Apollo graph.

        ## Graph Information
        - **Graph ID**: {graph_id}
        - **Variant**: {variant_name}
        - **Organization**: {organization_name}
        - **Graph Reference**: {graph_ref_str}
        {subgraph_line}

        ## Quick Start

        1. Build the MCP server:
           ```bash
           docker build -f mcp.Dockerfile -t {docker_tag}-mcp .
           ```

        2. Run the MCP server:
           ```bash
           {docker_run_hint}
           # Linux users may need: docker run --network=host --env-file .env {docker_tag}-mcp
           ```

        3. Test with MCP Inspector:
           ```bash
           npx @mcp/inspector
           ```

        4. Configure Claude Desktop to use this server.

        ## View in Apollo Studio
        {graph_endpoint}"#,
        graph_name = selected_graph.graph_name,
        graph_id = selected_graph.graph_id,
        variant_name = selected_graph.variant_name,
        organization_name = selected_graph.organization_name,
        docker_run_hint = mcp_docker_run_hint(docker_tag),
    )
}

/// Context for processing MCP template placeholders
pub struct MCPTemplateContext<'a> {
    pub project_name: &'a str,
    pub graph_id: &'a str,
    pub graph_name: &'a str,
    pub variant_name: &'a str,
    pub organization_name: &'a str,
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

/// Fills the Apollo API key placeholders in the content of a `.env` file.
///
/// The `.env` file generated from `.env.template` is the only file that receives the API key;
/// other generated files read it from the environment.
pub fn hydrate_env_api_key(content: &str, api_key: &str) -> String {
    content
        .replace("${APOLLO_KEY}", api_key)
        .replace("${APOLLO_API_KEY}", api_key)
        .replace("{{APOLLO_KEY}}", api_key)
        .replace("{{APOLLO_API_KEY}}", api_key)
}

/// Returns the path to write a template file to when it is the `.env` template.
///
/// A file named `.env.template`, in any directory, is written as `.env` next to it, and it is the
/// only file that receives the API key. Returns `None` for every other file.
pub fn env_output_path(template_path: &Utf8Path) -> Option<Utf8PathBuf> {
    (template_path.file_name() == Some(".env.template"))
        .then(|| template_path.with_file_name(".env"))
}

/// Returns `true` when `path` names a generated `.env` file.
pub fn is_env_file(path: &Utf8Path) -> bool {
    path.file_name() == Some(".env")
}

/// Writes one generated project file.
///
/// A `.env` file is written so that only its owner can read or write it. Other files are written
/// with the default permissions.
pub fn write_project_file(path: &Utf8Path, content: impl AsRef<[u8]>) -> std::io::Result<()> {
    if is_env_file(path) {
        write_owner_only_file(path, content.as_ref())
    } else {
        std::fs::write(path, content)
    }
}

/// Writes `content` to `path` so that only the owner can read or write it.
///
/// On Unix, a new file is created with mode `0600`, and an existing file is restricted to `0600`
/// before the new content is written, so other users cannot open the file and read the new
/// content. On other platforms this is a plain write.
pub fn write_owner_only_file(path: &Utf8Path, content: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::{
            io::Write,
            os::unix::fs::{OpenOptionsExt, PermissionsExt},
        };

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        // `mode` applies only when the file is created, so also restrict a file that already exists.
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(content)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, content)
    }
}

/// Values used to fill placeholders in `add-mcp` files when adding MCP to an existing graph
pub struct ExistingGraphMCPContext<'a> {
    pub project_name: &'a str,
    pub graph_ref: &'a str,
    pub graph_id: &'a str,
    pub graph_name: &'a str,
    pub variant_name: &'a str,
    pub organization_name: &'a str,
    pub api_key: &'a str,
    pub graph_studio_url: &'a str,
    pub mcp_server_binary: &'a str,
    pub mcp_config_path: &'a str,
    pub tools_path: &'a str,
}

/// Fills placeholders in one `add-mcp` file for an existing graph.
///
/// Returns the path to write the file to, relative to the project directory, and its content.
/// `.env.template` is written as `.env`, and it is the only file that receives the API key.
pub fn hydrate_existing_graph_file(
    file_path: &Utf8Path,
    content: &str,
    ctx: &ExistingGraphMCPContext,
) -> (Utf8PathBuf, String) {
    // Replace template placeholders with selected graph information
    // Use ${} format for YAML files (avoids linting issues)
    // Use {{}} format for other templates and conditionals
    let processed_content = content
        // ${} format - primarily for YAML files
        .replace("${PROJECT_NAME}", ctx.project_name)
        .replace("${GRAPH_REF}", ctx.graph_ref)
        .replace("${GRAPH_ID}", ctx.graph_id)
        .replace("${GRAPH_NAME}", ctx.graph_name)
        .replace("${VARIANT_NAME}", ctx.variant_name)
        .replace("${ORGANIZATION_NAME}", ctx.organization_name)
        .replace("${APOLLO_GRAPH_REF}", ctx.graph_ref)
        .replace("${GRAPHQL_ENDPOINT}", "http://localhost:4000")
        .replace("${STAGING_GRAPHQL_ENDPOINT}", "http://localhost:4000") // For staging YAML
        .replace("${GRAPH_STUDIO_URL}", ctx.graph_studio_url)
        .replace("${PROJECT_VERSION}", "1.0.0")
        .replace(
            "${PROJECT_REPOSITORY_URL}",
            &format!("https://github.com/user/{}", ctx.project_name),
        )
        // {{}} format - for non-YAML templates and backwards compatibility
        .replace("{{PROJECT_NAME}}", ctx.project_name)
        .replace("{{GRAPH_REF}}", ctx.graph_ref)
        .replace("{{GRAPH_ID}}", ctx.graph_id)
        .replace("{{GRAPH_NAME}}", ctx.graph_name)
        .replace("{{VARIANT_NAME}}", ctx.variant_name)
        .replace("{{ORGANIZATION_NAME}}", ctx.organization_name)
        .replace("{{APOLLO_GRAPH_REF}}", ctx.graph_ref)
        .replace("{{MCP_SERVER_BINARY}}", ctx.mcp_server_binary)
        .replace("{{MCP_CONFIG_PATH}}", ctx.mcp_config_path)
        .replace("{{GRAPHQL_ENDPOINT}}", "http://localhost:4000")
        .replace("{{GRAPH_STUDIO_URL}}", ctx.graph_studio_url)
        .replace("{{PROJECT_VERSION}}", "1.0.0")
        .replace(
            "{{PROJECT_REPOSITORY_URL}}",
            &format!("https://github.com/user/{}", ctx.project_name),
        )
        // Other replacements
        .replace("- /tools", &format!("- {}", ctx.tools_path))
        .replace(
            "endpoint: http://host.docker.internal:4000",
            "endpoint: http://localhost:4000",
        );

    // Handle the specific case where .env.template needs dynamic values populated
    if let Some(env_path) = env_output_path(file_path) {
        // Fill the quoted values in .env.template, then the API key
        let env_content = processed_content
            .replace("\"{{PROJECT_NAME}}\"", &format!("\"{}\"", ctx.project_name))
            .replace(
                "\"{{GRAPHQL_ENDPOINT}}\"",
                "\"http://host.docker.internal:4000/graphql\"",
            )
            .replace(
                "\"{{APOLLO_GRAPH_REF}}\"",
                &format!("\"{}\"", ctx.graph_ref),
            );

        // Write processed template as .env file instead of .env.template
        return (env_path, hydrate_env_api_key(&env_content, ctx.api_key));
    }

    (file_path.to_path_buf(), processed_content)
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
