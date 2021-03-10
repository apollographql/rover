use std::collections::HashMap;
use std::fmt::Debug;

use ansi_term::Colour::Yellow;
use atty::Stream;
use rover_client::query::subgraph::list::ListDetails;

use crate::utils::table::{self, cell, row};

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
    DocsList(HashMap<&'static str, &'static str>),
    SDL(String),
    SchemaHash(String),
    SubgraphList(ListDetails),
    Profiles(Vec<String>),
    None,
}

impl RoverStdout {
    pub fn print(&self) {
        match self {
            RoverStdout::DocsList(shortlinks) => {
                eprintln!(
                    "You can open any of these documentation pages by running {}.\n",
                    Yellow.normal().paint("`rover docs open <slug>`")
                );
                let mut table = table::get_table();

                // bc => sets top row to be bold and center
                table.add_row(row![bc => "Slug", "Description"]);
                for (shortlink_slug, shortlink_description) in shortlinks {
                    table.add_row(row![shortlink_slug, shortlink_description]);
                }
                println!("{}", table);
            }
            RoverStdout::SDL(sdl) => {
                if atty::is(Stream::Stdout) {
                    eprintln!("SDL:");
                }
                println!("{}", &sdl);
            }
            RoverStdout::SchemaHash(hash) => {
                if atty::is(Stream::Stdout) {
                    eprint!("Schema Hash: ");
                }
                println!("{}", &hash);
            }
            RoverStdout::SubgraphList(details) => {
                let mut subgraph_info = Vec::<(String, String, String)>::new();
                for subgraph in &details.subgraphs {
                    // if the url is None or empty (""), then set it to "N/A"
                    let url = subgraph.url.clone().unwrap_or_else(|| "N/A".to_string());
                    let url = if url.is_empty() {
                        "N/A".to_string()
                    } else {
                        url
                    };
                    let formatted_updated_at: String = if let Some(dt) = subgraph.updated_at {
                        dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
                    } else {
                        "N/A".to_string()
                    };

                    subgraph_info.push((subgraph.name.to_string(), url, formatted_updated_at));
                }

                if atty::is(Stream::Stdout) {
                    let mut table = table::get_table();

                    // bc => sets top row to be bold and center
                    table.add_row(row![bc => "Name", "Routing Url", "Last Updated"]);

                    for (name, url, updated_at) in &subgraph_info {
                        table.add_row(row![name, url, updated_at]);
                    }

                    println!("{}", table);
                } else {
                    println!("Name, Routing Url, Last Updated");
                    for (name, url, updated_at) in &subgraph_info {
                        println!("{}, {}, {}", name, url, updated_at)
                    }
                }

                eprintln!(
                    "View full details at {}/graph/{}/service-list",
                    details.root_url, details.graph_name
                );
            }
            RoverStdout::Profiles(profiles) => {
                if profiles.is_empty() {
                    eprintln!("No profiles found.");
                } else if atty::is(Stream::Stdout) {
                    eprintln!("Profiles:")
                }

                for profile in profiles {
                    println!("{}", profile);
                }
            }
            RoverStdout::None => (),
        }
    }
}
