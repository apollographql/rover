use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display};

use crate::utils::table::{self, cell, row};
use ansi_term::{Colour::Yellow, Style};
use atty::Stream;
use crossterm::style::Attribute::Underlined;
use rover_client::query::subgraph::list::ListDetails;
use termimad::MadSkin;

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
    SupergraphSdl(String),
    Sdl(String),
    CoreSchema(String),
    SchemaHash(String),
    SubgraphList(ListDetails),
    VariantList(Vec<String>),
    Profiles(Vec<String>),
    Introspection(String),
    Markdown(String),
    PlainText(String),
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
            RoverStdout::SupergraphSdl(sdl) => {
                print_descriptor("Supergraph SDL");
                print_content(&sdl);
            }
            RoverStdout::Sdl(sdl) => {
                print_descriptor("SDL");
                print_content(&sdl);
            }
            RoverStdout::CoreSchema(csdl) => {
                print_descriptor("CoreSchema");
                print_content(&csdl);
            }
            RoverStdout::SchemaHash(hash) => {
                print_one_line_descriptor("Schema Hash");
                print_content(&hash);
            }
            RoverStdout::SubgraphList(details) => {
                let mut table = table::get_table();

                // bc => sets top row to be bold and center
                table.add_row(row![bc => "Name", "Routing Url", "Last Updated"]);

                for subgraph in &details.subgraphs {
                    // Default to "unspecified" if the url is None or empty.
                    let url = subgraph
                        .url
                        .clone()
                        .unwrap_or_else(|| "unspecified".to_string());
                    let url = if url.is_empty() {
                        "unspecified".to_string()
                    } else {
                        url
                    };
                    let formatted_updated_at: String = if let Some(dt) = subgraph.updated_at {
                        dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
                    } else {
                        "N/A".to_string()
                    };

                    table.add_row(row![subgraph.name, url, formatted_updated_at]);
                }

                println!("{}", table);
                println!(
                    "View full details at {}/graph/{}/service-list",
                    details.root_url, details.graph_name
                );
            }
            RoverStdout::VariantList(variants) => {
                print_descriptor("Variants");
                for variant in variants {
                    println!("{}", variant);
                }
            }
            RoverStdout::Profiles(profiles) => {
                if profiles.is_empty() {
                    eprintln!("No profiles found.");
                } else {
                    print_descriptor("Profiles")
                }

                for profile in profiles {
                    println!("{}", profile);
                }
            }
            RoverStdout::Introspection(introspection_response) => {
                print_descriptor("Introspection Response");
                print_content(&introspection_response);
            }
            RoverStdout::Markdown(markdown_string) => {
                // underline bolded md
                let mut skin = MadSkin::default();
                skin.bold.add_attr(Underlined);

                println!("{}", skin.inline(&markdown_string));
            }
            RoverStdout::PlainText(text) => {
                println!("{}", text);
            }
            RoverStdout::None => (),
        }
    }
}

fn print_descriptor(descriptor: impl Display) {
    if atty::is(Stream::Stdout) {
        eprintln!("{}: \n", Style::new().bold().paint(descriptor.to_string()));
    }
}
fn print_one_line_descriptor(descriptor: impl Display) {
    if atty::is(Stream::Stdout) {
        eprint!("{}: ", Style::new().bold().paint(descriptor.to_string()));
    }
}

/// if the user is outputting to a terminal, we want there to be a terminating
/// newline, but we don't want that newline to leak into output that's piped
/// to a file, like from a `graph fetch`
fn print_content(content: impl Display) {
    if atty::is(Stream::Stdout) {
        println!("{}", content)
    } else {
        print!("{}", content)
    }
}
