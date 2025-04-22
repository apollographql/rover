use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::{hyperlink, successln, Style};

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

pub fn display_project_created_message(
    project_name: &str,
    artifacts: &[Utf8PathBuf],
    graph_ref: &GraphRef,
    api_key: &str,
) {
    println!();
    println!("{} All set! Your graph `{}` has been created. Please review details below to see what was generated.", Style::InfoPrefix.paint("=>"), Style::File.paint(project_name));
    println!();
    println!("{}", Style::Heading.paint("Files created:"));

    for artifact in artifacts.iter().filter(|a| !a.as_str().is_empty()) {
        successln!("{}", artifact);
    }
    println!();
    println!(
        "{}",
        Style::Heading.paint("GraphOS credentials for your graph")
    );
    successln!(
        "{}={} (Formatted graph-id@variant, references a graph in the Apollo GraphOS platform)",
        Style::Command.paint("APOLLO_GRAPH_REF"),
        graph_ref
    );
    successln!(
        "{}={} (This is your graph’s API key)",
        Style::Command.paint("APOLLO_KEY"),
        api_key
    );
    println!();
    println!("{}", Style::WarningHeading.paint("️▲ Before you proceed:"));
    println!("- Store your graph API key securely, you won’t be able to access it again!");
    println!();
    println!("{}", Style::Heading.paint("Next steps"));
    println!("Run the following command to start a local development session:");
    println!();
    println!(
        "{}",
        Style::Command.paint(format!(
            "APOLLO_KEY={} rover dev --graph-ref {} --supergraph-config supergraph.yaml",
            api_key, graph_ref
        ))
    );
    println!();
    println!("For more information, check out `getting-started.md`.");
    println!();
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
