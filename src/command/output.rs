use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display};

use crate::utils::table::{self, cell, row};
use ansi_term::Colour::{Cyan, Yellow};
use atty::Stream;
use crossterm::style::Attribute::*;
use regex::Regex;
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
            RoverStdout::Sdl(sdl) => {
                print_descriptor("SDL");
                println!("{}", &sdl);
            }
            RoverStdout::CoreSchema(csdl) => {
                print_descriptor("CoreSchema");
                println!("{}", &csdl);
            }
            RoverStdout::SchemaHash(hash) => {
                print_descriptor("Schema Hash");
                println!("{}", &hash);
            }
            RoverStdout::SubgraphList(details) => {
                let mut table = table::get_table();

                // bc => sets top row to be bold and center
                table.add_row(row![bc => "Name", "Routing Url", "Last Updated"]);

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
                println!("{}", &introspection_response);
            }
            RoverStdout::Markdown(markdown_string) => {
                // underline bolded md
                let mut skin = MadSkin::default();
                skin.bold.add_attr(Underlined);

                // replace links in format `[this](url)` to `this (url)`, since
                // termimad doesn't handle links for us.

                // this pattern captures the named groups, <title> and <url_with_parens>
                // that we can use to replace with later
                let re = Regex::new(r"\[(?P<title>[^\[]+)\](?P<url_with_parens>\(.*\))").unwrap();
                // we want to paint the replaced url cyan
                // the $pattern labels in the replacer match the <pattern>s in the regex above
                let replacer = format!("$title {}", Cyan.normal().paint("$url_with_parens"));
                let reformatted_urls = re.replace_all(markdown_string, replacer);

                println!("{}", skin.inline(&reformatted_urls));
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
        eprintln!("{}: ", descriptor);
    }
}
