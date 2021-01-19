use atty::{self, Stream};
use prettytable::{cell, row, Table};
use rover_client::query::subgraph::list::ListDetails;

/// RoverStdout defines all of the different types of data that are printed
/// to `stdout`. Every one of Rover's commands should return `anyhow::Result<RoverStdout>`
/// If the command needs to output some type of data, it should be structured
/// in this enum, and its print logic should be handled in `RoverStdout::print`
///
/// Not all commands will output machine readable information, and those should
/// return `Ok(RoverStdout::None)`. If a new command is added and it needs to
/// return something that is not described well in this enum, it should be added.
#[derive(Clone, PartialEq, Debug)]
pub enum RoverStdout {
    SDL(String),
    SchemaHash(String),
    SubgraphList(ListDetails),
    None,
}

impl RoverStdout {
    pub fn print(&self) {
        match self {
            RoverStdout::SDL(sdl) => {
                // we check to see if stdout is redirected
                // if it is, we don't print the content descriptor
                // this is because it would look strange to see
                // SDL:
                // and nothing after the colon if you piped the output
                // to another process or a file.
                if atty::is(Stream::Stdout) {
                    tracing::info!("SDL:");
                }
                println!("{}", &sdl);
            }
            RoverStdout::SchemaHash(hash) => {
                if atty::is(Stream::Stdout) {
                    tracing::info!("Schema Hash:");
                }
                println!("{}", &hash);
            }
            RoverStdout::SubgraphList(details) => {
                if atty::is(Stream::Stdout) {
                    tracing::info!("Subgraphs:");
                }

                let mut table = Table::new();
                table.add_row(row!["Name", "Routing Url", "Last Updated"]);

                for subgraph in &details.subgraphs {
                    table.add_row(row![
                        subgraph.name,
                        subgraph.url.clone().unwrap_or_else(|| "".to_string()),
                        subgraph.updated_at
                    ]);
                }

                println!("{}", table);
                println!(
                    "View full details at {}/graph/{}/service-list",
                    details.root_url, details.graph_name
                );
            }
            RoverStdout::None => (),
        }
    }
}
