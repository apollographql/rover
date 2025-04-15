use crate::command::init::graph_id::validation::GraphId;
use camino::Utf8PathBuf;
use rover_std::Style;

pub fn display_welcome_message() {
    println!(
        "\nWelcome! This command helps you initialize a federated Graph in your current directory."
    );
    println!(
        "\nTo learn more about init, run `{}` or visit {}\n",
        Style::Command.paint("rover init -h"),
        Style::Link.paint("https://www.apollographql.com/docs/rover/commands/init")
    );
}

pub fn display_project_created_message(
    project_name: &str,
    artifacts: &Vec<Utf8PathBuf>,
    graph_id: &GraphId,
    api_key: &str,
) {
    println!("\n=> All set! Your project `{}` has been created. Please review details below to see what was generated.", project_name);

    // Display created files
    println!("\nProject directory");
    for artifact in artifacts {
        println!("✓ {}", artifact);
    }

    // Display credentials
    println!("\n{}", Style::Heading.paint("GraphOS credentials"));
    println!("{}: {}", Style::Command.paint("Graph"), graph_id);
    println!("{}: {}}", Style::Command.paint("API Key", api_key));

    println!("{}", Style::WarningHeading.paint("Before you proceed:"));
    println!("- Set your graph API key as an environment variable; learn more about env vars by running `rover docs open configuring`");
    println!("- Save your graph ref (You can also get it from Studio by visiting your graph variant's home page)");

    println!("\n{}", Style::Heading.paint("Next steps"));
    println!("Run the following command to start a local development session:\n");
    println!(
        "{}",
        Style::Command.paint("$ rover dev --supergraph-config supergraph.yaml\n")
    );
    println!("For more information, check out `getting-started.md`.");
}

pub fn display_use_template_message() {
    println!();
    println!(
        "To add a new subgraph to an existing API project, use `{}`.",
        Style::Command.paint("rover template")
    );
    println!(
        "To learn more about templates, run `{}`",
        Style::Link.paint("rover docs open template")
    );
}
