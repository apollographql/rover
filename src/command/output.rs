use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display};

use crate::utils::table::{self, cell, row};

use ansi_term::{Colour::Yellow, Style};
use atty::Stream;
use crossterm::style::Attribute::Underlined;
use rover_client::operations::subgraph::list::SubgraphListResponse;
use rover_client::shared::{CheckResponse, FetchResponse, SdlType};
use serde_json::{json, Value};
use termimad::MadSkin;

/// RoverOutput defines all of the different types of data that are printed
/// to `stdout`. Every one of Rover's commands should return `anyhow::Result<RoverOutput>`
/// If the command needs to output some type of data, it should be structured
/// in this enum, and its print logic should be handled in `RoverOutput::print`
///
/// Not all commands will output machine readable information, and those should
/// return `Ok(RoverOutput::None)`. If a new command is added and it needs to
/// return something that is not described well in this enum, it should be added.
#[derive(Clone, PartialEq, Debug)]
pub enum RoverOutput {
    DocsList(HashMap<&'static str, &'static str>),
    FetchResponse(FetchResponse),
    CoreSchema(String),
    SchemaHash(String),
    SubgraphList(SubgraphListResponse),
    CheckResponse(CheckResponse),
    VariantList(Vec<String>),
    Profiles(Vec<String>),
    Introspection(String),
    Markdown(String),
    None,
}

impl RoverOutput {
    pub fn print(&self) {
        match self {
            RoverOutput::DocsList(shortlinks) => {
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
            RoverOutput::FetchResponse(fetch_response) => {
                match fetch_response.sdl.r#type {
                    SdlType::Graph | SdlType::Subgraph => print_descriptor("SDL"),
                    SdlType::Supergraph => print_descriptor("Supergraph SDL"),
                }
                print_content(&fetch_response.sdl.contents);
            }
            RoverOutput::CoreSchema(csdl) => {
                print_descriptor("CoreSchema");
                print_content(&csdl);
            }
            RoverOutput::SchemaHash(hash) => {
                print_one_line_descriptor("Schema Hash");
                print_content(&hash);
            }
            RoverOutput::SubgraphList(details) => {
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
                    let formatted_updated_at: String = if let Some(dt) = subgraph.updated_at.local {
                        dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
                    } else {
                        "N/A".to_string()
                    };

                    table.add_row(row![subgraph.name, url, formatted_updated_at]);
                }

                println!("{}", table);
                println!(
                    "View full details at {}/graph/{}/service-list",
                    details.root_url, details.graph_ref.name
                );
            }
            RoverOutput::CheckResponse(check_response) => {
                print_check_response(check_response);
            }
            RoverOutput::VariantList(variants) => {
                print_descriptor("Variants");
                for variant in variants {
                    println!("{}", variant);
                }
            }
            RoverOutput::Profiles(profiles) => {
                if profiles.is_empty() {
                    eprintln!("No profiles found.");
                } else {
                    print_descriptor("Profiles")
                }

                for profile in profiles {
                    println!("{}", profile);
                }
            }
            RoverOutput::Introspection(introspection_response) => {
                print_descriptor("Introspection Response");
                print_content(&introspection_response);
            }
            RoverOutput::Markdown(markdown_string) => {
                // underline bolded md
                let mut skin = MadSkin::default();
                skin.bold.add_attr(Underlined);

                println!("{}", skin.inline(&markdown_string));
            }
            RoverOutput::None => (),
        }
    }

    pub fn get_internal_json(&self) -> Option<Value> {
        match self {
            RoverOutput::DocsList(shortlinks) => {
                let mut shortlink_vec = vec![];
                for (shortlink_slug, shortlink_description) in shortlinks {
                    shortlink_vec.push(
                        json!({"slug": shortlink_slug, "description": shortlink_description }),
                    );
                }
                Some(json!({ "shortlinks": shortlink_vec }))
            }
            RoverOutput::FetchResponse(fetch_response) => Some(json!(fetch_response)),
            RoverOutput::CoreSchema(csdl) => Some(json!({ "core_schema": csdl })),
            RoverOutput::SchemaHash(hash) => Some(json!({ "schema_hash": hash })),
            RoverOutput::SubgraphList(list_response) => Some(json!(list_response)),
            RoverOutput::CheckResponse(check_response) => Some(json!(check_response)),
            RoverOutput::VariantList(variants) => Some(json!({ "variants": variants })),
            RoverOutput::Profiles(profiles) => Some(json!({ "profiles": profiles })),
            RoverOutput::Introspection(introspection_response) => {
                Some(json!({ "introspection_response": introspection_response }))
            }
            RoverOutput::Markdown(markdown_string) => Some(json!({ "markdown": markdown_string })),
            RoverOutput::None => None,
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

pub(crate) fn print_check_response(check_response: &CheckResponse) {
    let num_changes = check_response.changes.len();

    let msg = match num_changes {
        0 => "There were no changes detected in the composed schema.".to_string(),
        _ => format!(
            "Compared {} schema changes against {} operations",
            num_changes, check_response.number_of_checked_operations
        ),
    };

    eprintln!("{}", &msg);

    if !check_response.changes.is_empty() {
        let mut table = table::get_table();

        // bc => sets top row to be bold and center
        table.add_row(row![bc => "Change", "Code", "Description"]);
        for check in &check_response.changes {
            table.add_row(row![check.severity, check.code, check.description]);
        }

        print_content(&table);
    }

    if let Some(url) = &check_response.target_url {
        eprintln!("View full details at {}", url);
    }
}
