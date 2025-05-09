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

pub fn generate_project_created_message(
    project_name: String,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: String,
    commands: Option<Vec<String>>,
    start_point_file: String,
    #[cfg(feature = "init")] print_depth: Option<u8>,
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
    let dev_command = if !artifacts.is_empty() {
        format!(
            "APOLLO_KEY={} rover dev --graph-ref {} --supergraph-config supergraph.yaml",
            api_key, graph_ref
        )
    } else {
        format!("APOLLO_KEY={} rover dev --graph-ref {}", api_key, graph_ref)
    };

    if let Some(commands) = commands {
        // If commands vec is not empty, display the commands
        if !commands.is_empty() {
            // Filter out empty commands and enumerate the valid ones
            for (i, cmd) in commands
                .iter()
                .filter(|cmd| !cmd.trim().is_empty())
                .enumerate()
            {
                output.push_str(&format!(
                    "{}) Run: {}\n",
                    i + 1,
                    Style::Command.paint(cmd.trim())
                ));
            }

            // Number the development command after all valid commands
            // i.e. if we had 2 valid commands, it will be numbered 3)
            let next_number = commands.iter().filter(|cmd| !cmd.trim().is_empty()).count() + 1;
            output.push_str(&format!(
                "{}) Start a local development session: {}\n",
                next_number,
                Style::Command.paint(dev_command)
            ));
        } else {
            // If commands vec is empty, just show the rover dev command
            output.push_str("Start a local development session:\n");
            output.push_str(&format!("{}\n", Style::Command.paint(dev_command)));
        }
    } else {
        // If no command is provided, just show the rover dev command
        output.push_str("Start a local development session:\n");
        output.push_str(&format!("{}\n", Style::Command.paint(dev_command)));
    }

    output.push_str(&format!(
        "\nFor more information, check out '{}'.\n\n",
        start_point_file
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
    #[cfg(feature = "init")] print_depth: Option<u8>,
) {
    #[cfg(feature = "init")]
    let message = generate_project_created_message(
        project_name,
        artifacts,
        graph_ref,
        api_key,
        commands,
        start_point_file,
        print_depth,
    );
    println!("{}", message);
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
