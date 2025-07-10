use crate::command::init::template_operations::print_grouped_files;
use crate::command::init::template_operations::PrintMode::Confirmation;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::{hyperlink, Style};

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

#[cfg(feature = "react-template")]
pub fn generate_react_project_created_message(
    project_name: String,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: String,
    commands: Option<Vec<String>>,
    start_point_file: String,
    print_depth: Option<u8>,
    organization_id: &str,
) -> String {
    // Add welcome message
    println!(
        "\nAll set! Your React app '{}' has been created. Please review details below to see what was generated.\n",
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
        .push_str("- Store your graph API key securely, you won't be able to access it again!\n");
    output
        .push_str("- Add your credentials to your React app's environment configuration\n\n");

    // Add next steps section for React apps
    output.push_str(&format!("{}\n", Style::Heading.paint("Next steps")));
    
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
                    .push_str("1) Install dependencies and start your React development server:\n\n");
                output.push_str(&format!("{}\n", Style::Command.paint(valid_commands[0])));
            } else {
                output.push_str(
                    "1) Install dependencies and start your React development server:\n\n",
                );
                for cmd in valid_commands {
                    output.push_str(&format!("{}\n", Style::Command.paint(cmd)));
                }
            }
        } else {
            // Default React development commands
            output.push_str("1) Install dependencies and start your React development server:\n\n");
            output.push_str(&format!("{}\n", Style::Command.paint("npm install")));
            output.push_str(&format!("{}\n", Style::Command.paint("npm run dev")));
        }
    } else {
        // Default React development commands
        output.push_str("1) Install dependencies and start your React development server:\n\n");
        output.push_str(&format!("{}\n", Style::Command.paint("npm install")));
        output.push_str(&format!("{}\n", Style::Command.paint("npm run dev")));
    }

    output.push_str("\n2) Configure your Apollo Client with the GraphOS credentials above\n");
    output.push_str("3) Start building your React app with GraphQL queries!\n");

    // Add GraphOS Studio integration section
    output.push_str(&format!("\n{}\n", Style::Heading.paint("GraphOS Studio Integration")));
    output.push_str(&format!(
        "Visit {} to finish setup and leverage all the benefits of GraphOS:\n\n",
        Style::Link.paint(format!("https://studio.apollographql.com/org/{}/graphs", organization_id))
    ));
    output.push_str("Congratulations! You now have:\n");
    output.push_str("- Schema registry and versioning\n");
    output.push_str("- Performance monitoring\n");
    output.push_str("- Breaking change detection\n");
    output.push_str("- Field usage analytics\n");
    output.push_str("- Operation safety checks\n");
    output.push_str("- AI generated mocking\n");

    output.push_str(&format!(
        "\nFor more information, check out '{start_point_file}'.\n\n"
    ));

    output
}

#[cfg(feature = "react-template")]
pub fn generate_react_project_no_graph_message(
    project_name: String,
    artifacts: &[Utf8PathBuf],
    commands: Option<Vec<String>>,
    start_point_file: String,
    print_depth: Option<u8>,
    organization_id: &str,
) -> String {
    // Add welcome message
    println!(
        "\nAll set! Your React app '{}' has been created. Please review details below to see what was generated.\n",
        Style::File.paint(project_name)
    );

    print_grouped_files(artifacts.to_vec(), print_depth, Confirmation);

    println!();

    let mut output = String::new();
    
    // Add information section for React apps without graphs
    output.push_str(&format!(
        "{}\n",
        Style::Heading.paint("React App Created Successfully")
    ));
    output.push_str("Your React TypeScript application with Apollo Client has been set up locally.\n");
    output.push_str("No GraphOS graph was created - you can connect to any existing GraphQL endpoint.\n\n");

    // Add warning section
    output.push_str(&format!(
        "{}\n",
        Style::WarningHeading.paint("️▲ Before you proceed:")
    ));
    output.push_str("- Configure your Apollo Client to connect to your GraphQL endpoint\n");
    output.push_str("- Update the VITE_GRAPHQL_ENDPOINT in your .env file\n\n");

    // Add next steps section for React apps
    output.push_str(&format!("{}\n", Style::Heading.paint("Next steps")));
    
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
                    .push_str("1) Install dependencies and start your React development server:\n\n");
                output.push_str(&format!("{}\n", Style::Command.paint(valid_commands[0])));
            } else {
                output.push_str(
                    "1) Install dependencies and start your React development server:\n\n",
                );
                for cmd in valid_commands {
                    output.push_str(&format!("{}\n", Style::Command.paint(cmd)));
                }
            }
        } else {
            // Default React development commands
            output.push_str("1) Install dependencies and start your React development server:\n\n");
            output.push_str(&format!("{}\n", Style::Command.paint("npm install")));
            output.push_str(&format!("{}\n", Style::Command.paint("npm run dev")));
        }
    } else {
        // Default React development commands
        output.push_str("1) Install dependencies and start your React development server:\n\n");
        output.push_str(&format!("{}\n", Style::Command.paint("npm install")));
        output.push_str(&format!("{}\n", Style::Command.paint("npm run dev")));
    }

    output.push_str("\n2) Configure your Apollo Client to connect to your GraphQL endpoint\n");
    output.push_str("3) Start building your React app with GraphQL queries!\n");

    // Add optional GraphOS setup section
    output.push_str(&format!("\n{}\n", Style::Heading.paint("Optional: Connect to GraphOS")));
    output.push_str("If you want to connect to Apollo GraphOS later, you can:\n");
    output.push_str("- Create a graph in Apollo Studio\n");
    output.push_str("- Get your graph credentials\n");
    output.push_str("- Update your Apollo Client configuration\n");
    output.push_str(&format!(
        "\nVisit {} to create a graph when you're ready.\n",
        Style::Link.paint(format!("https://studio.apollographql.com/org/{}/graphs", organization_id))
    ));

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
    is_react_template: bool,
    #[cfg_attr(not(feature = "react-template"), allow(unused_variables))]
    organization_id: Option<&str>,
    #[cfg_attr(not(feature = "react-template"), allow(unused_variables))]
    graph_created: bool,
) {
    let message = if is_react_template {
        #[cfg(feature = "react-template")]
        {
            if graph_created {
                generate_react_project_created_message(
                    project_name,
                    artifacts,
                    graph_ref,
                    api_key,
                    commands,
                    start_point_file,
                    print_depth,
                    organization_id.unwrap_or(""),
                )
            } else {
                generate_react_project_no_graph_message(
                    project_name,
                    artifacts,
                    commands,
                    start_point_file,
                    print_depth,
                    organization_id.unwrap_or(""),
                )
            }
        }
        #[cfg(not(feature = "react-template"))]
        {
            // Fallback to standard message when react-template feature is disabled
            generate_project_created_message(
                project_name,
                artifacts,
                graph_ref,
                api_key,
                commands,
                start_point_file,
                print_depth,
            )
        }
    } else {
        generate_project_created_message(
            project_name,
            artifacts,
            graph_ref,
            api_key,
            commands,
            start_point_file,
            print_depth,
        )
    };
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
