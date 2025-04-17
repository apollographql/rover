use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::{hyperlink, successln, Style};

pub fn display_welcome_message() {
    println!();
    println!(
        "Welcome! This command helps you initialize a federated Graph in your current directory."
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
    println!("{} All set! Your project `{}` has been created. Please review details below to see what was generated.", Style::InfoPrefix.paint("=>"), Style::File.paint(project_name));
    println!();
    println!("{}", Style::Heading.paint("Project directory"));

    for artifact in artifacts.iter().filter(|a| !a.as_str().is_empty()) {
        successln!("{}", artifact);
    }
    println!();
    println!(
        "{}",
        Style::Heading.paint("GraphOS credentials for your graph")
    );
    successln!("{}={}", Style::Command.paint("APOLLO_GRAPH_REF"), graph_ref);
    successln!("{}={}", Style::Command.paint("APOLLO_KEY"), api_key);
    println!();
    println!("{}", Style::WarningHeading.paint("️▲ Before you proceed:"));
    println!("- Set your graph API key as an environment variable; learn more about env vars by running {}",Style::Command.paint("`rover docs open configuring`"));
    println!("- Save your graph ref (You can also get it from Studio by visiting your graph variant's home page)");
    println!();
    println!("{}", Style::Heading.paint("Next steps"));
    println!("Run the following command to start a local development session:");
    println!();
    println!(
        "{}",
        Style::Command.paint("$ rover dev --supergraph-config supergraph.yaml")
    );
    println!();
    println!("For more information, check out `getting-started.md`.");
    println!();
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
