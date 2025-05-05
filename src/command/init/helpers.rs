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
    project_name: &str,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: &str,
    command: Option<&str>,
    start_point_file: &str,
) -> String {
    let mut output = String::new();

    // Add welcome message
    output.push_str(&format!(
        "\nAll set! Your graph '{}' has been created. Please review details below to see what was generated.\n\n",
        Style::File.paint(project_name)
    ));

    // Add files section only if there are artifacts
    if !artifacts.is_empty() {
        output.push_str(&format!("{}\n", Style::Heading.paint("Files created:")));
        for artifact in artifacts.iter().filter(|a| !a.as_str().is_empty()) {
            output.push_str(&format!("{}\n", Style::Success.paint(artifact)));
        }
        output.push('\n');
    }

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
    if let Some(command) = command {
        output.push_str(&format!(
            "1) Start the service: {}\n",
            Style::Command.paint(command)
        ));
        output.push_str(&format!(
            "2) Start a local development session: {}\n",
            Style::Command.paint(dev_command)
        ));
    } else {
        output.push_str("Start a local development session:\n");
        output.push_str(&format!("{}\n", Style::Command.paint(dev_command)));
    }
    output.push_str(&format!("\nFor more information, check out '{}'.\n\n", start_point_file));

    output
}

pub fn display_project_created_message(
    project_name: &str,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: &str,
    start_point_file: &str,
) {
    let message = generate_project_created_message(
        project_name,
        artifacts,
        graph_ref,
        api_key,
        None,
        start_point_file,
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
