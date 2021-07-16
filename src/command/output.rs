use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display};

use crate::utils::table::{self, cell, row};

use ansi_term::{
    Colour::{Red, Yellow},
    Style,
};
use atty::Stream;
use crossterm::style::Attribute::Underlined;
use rover_client::operations::graph::publish::GraphPublishResponse;
use rover_client::operations::subgraph::list::SubgraphListResponse;
use rover_client::operations::subgraph::publish::SubgraphPublishResponse;
use rover_client::shared::{CheckResponse, FetchResponse, GraphRef, SdlType};
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
    SubgraphList(SubgraphListResponse),
    CheckResponse(CheckResponse),
    GraphPublishResponse {
        graph_ref: GraphRef,
        publish_response: GraphPublishResponse,
    },
    SubgraphPublishResponse {
        graph_ref: GraphRef,
        subgraph: String,
        publish_response: SubgraphPublishResponse,
    },
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
            RoverOutput::GraphPublishResponse {
                graph_ref,
                publish_response,
            } => {
                eprintln!(
                    "{}#{} Published successfully {}",
                    graph_ref, publish_response.schema_hash, publish_response.change_summary
                );
                print_one_line_descriptor("Schema Hash");
                print_content(&publish_response.schema_hash);
            }
            RoverOutput::SubgraphPublishResponse {
                graph_ref,
                subgraph,
                publish_response,
            } => {
                if publish_response.subgraph_was_created {
                    eprintln!(
                        "A new subgraph called '{}' for the '{}' graph was created",
                        subgraph, graph_ref
                    );
                } else {
                    eprintln!(
                        "The '{}' subgraph for the '{}' graph was updated",
                        subgraph, graph_ref
                    );
                }

                if publish_response.did_update_gateway {
                    eprintln!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' subgraph", graph_ref, subgraph);
                } else {
                    eprintln!(
                        "The gateway for the '{}' graph was NOT updated with a new schema",
                        graph_ref
                    );
                }

                if !publish_response.composition_errors.errors.is_empty() {
                    let warn_prefix = Red.normal().paint("WARN:");
                    eprintln!("{} The following composition errors occurred:", warn_prefix,);
                    for error in &publish_response.composition_errors.errors {
                        eprintln!("{}", &error);
                    }
                }
            }
            RoverOutput::CoreSchema(csdl) => {
                print_descriptor("CoreSchema");
                print_content(&csdl);
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
                print_descriptor("Check Result");
                print_content(check_response.get_table());
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
            RoverOutput::GraphPublishResponse {
                graph_ref: _,
                publish_response,
            } => Some(json!(publish_response)),
            RoverOutput::SubgraphPublishResponse {
                graph_ref: _,
                subgraph: _,
                publish_response,
            } => Some(json!(publish_response)),
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
