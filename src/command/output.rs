use std::collections::BTreeMap;
use std::fmt::{self, Debug, Display};

use crate::error::RoverError;
use crate::utils::table::{self, cell, row};

use ansi_term::{
    Colour::{Cyan, Red, Yellow},
    Style,
};
use atty::Stream;
use crossterm::style::Attribute::Underlined;
use rover_client::operations::graph::publish::GraphPublishResponse;
use rover_client::operations::subgraph::delete::SubgraphDeleteResponse;
use rover_client::operations::subgraph::list::SubgraphListResponse;
use rover_client::operations::subgraph::publish::SubgraphPublishResponse;
use rover_client::shared::{CheckResponse, FetchResponse, GraphRef, SdlType};
use rover_client::RoverClientError;
use serde::Serialize;
use serde_json::{json, Value};
use termimad::MadSkin;

/// RoverOutput defines all of the different types of data that are printed
/// to `stdout`. Every one of Rover's commands should return `anyhow::Result<RoverOutput>`
/// If the command needs to output some type of data, it should be structured
/// in this enum, and its print logic should be handled in `RoverOutput::print`
///
/// Not all commands will output machine readable information, and those should
/// return `Ok(RoverOutput::EmptySuccess)`. If a new command is added and it needs to
/// return something that is not described well in this enum, it should be added.
#[derive(Clone, PartialEq, Debug)]
pub enum RoverOutput {
    DocsList(BTreeMap<&'static str, &'static str>),
    FetchResponse(FetchResponse),
    CoreSchema(String),
    CompositionResult {
        supergraph_sdl: String,
        hints: Vec<String>,
    },
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
    SubgraphDeleteResponse {
        graph_ref: GraphRef,
        subgraph: String,
        dry_run: bool,
        delete_response: SubgraphDeleteResponse,
    },
    Profiles(Vec<String>),
    Introspection(String),
    ErrorExplanation(String),
    EmptySuccess,
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
                    SdlType::Graph | SdlType::Subgraph { .. } => print_descriptor("SDL"),
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
                    graph_ref, publish_response.api_schema_hash, publish_response.change_summary
                );
                print_one_line_descriptor("Schema Hash");
                print_content(&publish_response.api_schema_hash);
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

                if publish_response.supergraph_was_updated {
                    eprintln!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' subgraph", graph_ref, subgraph);
                } else {
                    eprintln!(
                        "The gateway for the '{}' graph was NOT updated with a new schema",
                        graph_ref
                    );
                }

                if !publish_response.build_errors.is_empty() {
                    let warn_prefix = Red.normal().paint("WARN:");
                    eprintln!("{} The following build errors occurred:", warn_prefix,);
                    eprintln!("{}", &publish_response.build_errors);
                }
            }
            RoverOutput::SubgraphDeleteResponse {
                graph_ref,
                subgraph,
                dry_run,
                delete_response,
            } => {
                let warn_prefix = Red.normal().paint("WARN:");
                if *dry_run {
                    if !delete_response.build_errors.is_empty() {
                        eprintln!(
                            "{} Deleting the {} subgraph from {} would result in the following build errors:",
                            warn_prefix,
                            Cyan.normal().paint(subgraph),
                            Cyan.normal().paint(graph_ref.to_string()),
                        );

                        eprintln!("{}", &delete_response.build_errors);
                        eprintln!("{} This is only a prediction. If the graph changes before confirming, these errors could change.", warn_prefix);
                    } else {
                        eprintln!("{} At the time of checking, there would be no build errors resulting from the deletion of this subgraph.", warn_prefix);
                        eprintln!("{} This is only a prediction. If the graph changes before confirming, there could be build errors.", warn_prefix)
                    }
                } else {
                    if delete_response.supergraph_was_updated {
                        eprintln!(
                            "The {} subgraph was removed from {}. Remaining subgraphs were composed.",
                            Cyan.normal().paint(subgraph),
                            Cyan.normal().paint(graph_ref.to_string()),
                        )
                    } else {
                        eprintln!(
                            "{} The gateway for {} was not updated. See errors below.",
                            warn_prefix,
                            Cyan.normal().paint(graph_ref.to_string())
                        )
                    }

                    if !delete_response.build_errors.is_empty() {
                        eprintln!(
                            "{} There were build errors as a result of deleting the subgraph:",
                            warn_prefix,
                        );

                        eprintln!("{}", &delete_response.build_errors);
                    }
                }
            }
            RoverOutput::CoreSchema(csdl) => {
                print_descriptor("CoreSchema");
                print_content(&csdl);
            }
            RoverOutput::CompositionResult {
                supergraph_sdl,
                hints,
            } => {
                let warn_prefix = Red.normal().paint("WARN:");
                for hint in hints {
                    eprintln!("{} {}", warn_prefix, hint);
                }
                println!();
                print_descriptor("CoreSchema");
                print_content(&supergraph_sdl);
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
            RoverOutput::ErrorExplanation(explanation) => {
                // underline bolded md
                let mut skin = MadSkin::default();
                skin.bold.add_attr(Underlined);

                println!("{}", skin.inline(explanation));
            }
            RoverOutput::EmptySuccess => (),
        }
    }

    pub(crate) fn get_internal_data_json(&self) -> Value {
        match self {
            RoverOutput::DocsList(shortlinks) => {
                let mut shortlink_vec = Vec::with_capacity(shortlinks.len());
                for (shortlink_slug, shortlink_description) in shortlinks {
                    shortlink_vec.push(
                        json!({"slug": shortlink_slug, "description": shortlink_description }),
                    );
                }
                json!({ "shortlinks": shortlink_vec })
            }
            RoverOutput::FetchResponse(fetch_response) => json!(fetch_response),
            RoverOutput::CoreSchema(csdl) => json!({ "core_schema": csdl }),
            RoverOutput::CompositionResult {
                supergraph_sdl,
                hints,
            } => json!({
              "core_schema": supergraph_sdl,
              "hints": hints
            }),
            RoverOutput::GraphPublishResponse {
                graph_ref: _,
                publish_response,
            } => json!(publish_response),
            RoverOutput::SubgraphPublishResponse {
                graph_ref: _,
                subgraph: _,
                publish_response,
            } => json!(publish_response),
            RoverOutput::SubgraphDeleteResponse {
                graph_ref: _,
                subgraph: _,
                dry_run: _,
                delete_response,
            } => {
                json!(delete_response)
            }
            RoverOutput::SubgraphList(list_response) => json!(list_response),
            RoverOutput::CheckResponse(check_response) => check_response.get_json(),
            RoverOutput::Profiles(profiles) => json!({ "profiles": profiles }),
            RoverOutput::Introspection(introspection_response) => {
                json!({ "introspection_response": introspection_response })
            }
            RoverOutput::ErrorExplanation(explanation_markdown) => {
                json!({ "explanation_markdown": explanation_markdown })
            }
            RoverOutput::EmptySuccess => json!(null),
        }
    }

    pub(crate) fn get_internal_error_json(&self) -> Value {
        let rover_error = match self {
            RoverOutput::SubgraphPublishResponse {
                graph_ref,
                subgraph,
                publish_response,
            } => {
                if !publish_response.build_errors.is_empty() {
                    Some(RoverError::from(RoverClientError::SubgraphBuildErrors {
                        subgraph: subgraph.clone(),
                        graph_ref: graph_ref.clone(),
                        source: publish_response.build_errors.clone(),
                    }))
                } else {
                    None
                }
            }
            RoverOutput::SubgraphDeleteResponse {
                graph_ref,
                subgraph,
                dry_run: _,
                delete_response,
            } => {
                if !delete_response.build_errors.is_empty() {
                    Some(RoverError::from(RoverClientError::SubgraphBuildErrors {
                        subgraph: subgraph.clone(),
                        graph_ref: graph_ref.clone(),
                        source: delete_response.build_errors.clone(),
                    }))
                } else {
                    None
                }
            }
            _ => None,
        };
        json!(rover_error)
    }

    pub(crate) fn get_json_version(&self) -> JsonVersion {
        match &self {
            RoverOutput::CompositionResult { .. } => JsonVersion::OneAlpha,
            _ => JsonVersion::default(),
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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct JsonOutput {
    json_version: JsonVersion,
    data: JsonData,
    error: Value,
}

impl JsonOutput {
    pub(crate) fn success(data: Value, error: Value, json_version: JsonVersion) -> JsonOutput {
        JsonOutput {
            json_version,
            data: JsonData::success(data),
            error,
        }
    }

    pub(crate) fn failure(data: Value, error: Value, json_version: JsonVersion) -> JsonOutput {
        JsonOutput {
            json_version,
            data: JsonData::failure(data),
            error,
        }
    }
}

impl fmt::Display for JsonOutput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", json!(self))
    }
}

impl From<RoverError> for JsonOutput {
    fn from(error: RoverError) -> Self {
        let data_json = error.get_internal_data_json();
        let error_json = error.get_internal_error_json();
        JsonOutput::failure(data_json, error_json, error.get_json_version())
    }
}

impl From<RoverOutput> for JsonOutput {
    fn from(output: RoverOutput) -> Self {
        let data = output.get_internal_data_json();
        let error = output.get_internal_error_json();
        JsonOutput::success(data, error, output.get_json_version())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct JsonData {
    #[serde(flatten)]
    inner: Value,
    success: bool,
}

impl JsonData {
    pub(crate) fn success(inner: Value) -> JsonData {
        JsonData {
            inner,
            success: true,
        }
    }

    pub(crate) fn failure(inner: Value) -> JsonData {
        JsonData {
            inner,
            success: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) enum JsonVersion {
    #[serde(rename = "1")]
    One,
    #[serde(rename = "1.alpha")]
    OneAlpha,
}

impl Default for JsonVersion {
    fn default() -> Self {
        JsonVersion::One
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use assert_json_diff::assert_json_eq;
    use chrono::{DateTime, Local, Utc};
    use rover_client::{
        operations::{
            graph::publish::{ChangeSummary, FieldChanges, TypeChanges},
            subgraph::{
                delete::SubgraphDeleteResponse,
                list::{SubgraphInfo, SubgraphUpdatedAt},
            },
        },
        shared::{ChangeSeverity, SchemaChange, Sdl},
    };

    use apollo_federation_types::{BuildError, BuildErrors};

    use crate::anyhow;

    use super::*;

    #[test]
    fn docs_list_json() {
        let mut mock_shortlinks = BTreeMap::new();
        mock_shortlinks.insert("slug_one", "description_one");
        mock_shortlinks.insert("slug_two", "description_two");
        let actual_json: JsonOutput = RoverOutput::DocsList(mock_shortlinks).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "shortlinks": [
                    {
                        "slug": "slug_one",
                        "description": "description_one"
                    },
                    {
                        "slug": "slug_two",
                        "description": "description_two"
                    }
                ],
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn fetch_response_json() {
        let mock_fetch_response = FetchResponse {
            sdl: Sdl {
                contents: "sdl contents".to_string(),
                r#type: SdlType::Subgraph {
                    routing_url: Some("http://localhost:8000/graphql".to_string()),
                },
            },
        };
        let actual_json: JsonOutput = RoverOutput::FetchResponse(mock_fetch_response).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "sdl": {
                    "contents": "sdl contents",
                },
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn core_schema_json() {
        let mock_core_schema = "core schema contents".to_string();
        let actual_json: JsonOutput = RoverOutput::CoreSchema(mock_core_schema).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "core_schema": "core schema contents",
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_list_json() {
        let now_utc: DateTime<Utc> = Utc::now();
        let now_local: DateTime<Local> = now_utc.into();
        let mock_subgraph_list_response = SubgraphListResponse {
            subgraphs: vec![
                SubgraphInfo {
                    name: "subgraph one".to_string(),
                    url: Some("http://localhost:4001".to_string()),
                    updated_at: SubgraphUpdatedAt {
                        local: Some(now_local),
                        utc: Some(now_utc),
                    },
                },
                SubgraphInfo {
                    name: "subgraph two".to_string(),
                    url: None,
                    updated_at: SubgraphUpdatedAt {
                        local: None,
                        utc: None,
                    },
                },
            ],
            root_url: "https://studio.apollographql.com/".to_string(),
            graph_ref: GraphRef {
                name: "graph".to_string(),
                variant: "current".to_string(),
            },
        };
        let actual_json: JsonOutput = RoverOutput::SubgraphList(mock_subgraph_list_response).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "subgraphs": [
                    {
                        "name": "subgraph one",
                        "url": "http://localhost:4001",
                        "updated_at": {
                            "local": now_local,
                            "utc": now_utc
                        }
                    },
                    {
                        "name": "subgraph two",
                        "url": null,
                        "updated_at": {
                            "local": null,
                            "utc": null
                        }
                    }
                ],
                "success": true
          },
          "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_delete_success_json() {
        let mock_subgraph_delete = SubgraphDeleteResponse {
            supergraph_was_updated: true,
            build_errors: BuildErrors::new(),
        };
        let actual_json: JsonOutput = RoverOutput::SubgraphDeleteResponse {
            delete_response: mock_subgraph_delete,
            subgraph: "subgraph".to_string(),
            dry_run: false,
            graph_ref: GraphRef {
                name: "name".to_string(),
                variant: "current".to_string(),
            },
        }
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "supergraph_was_updated": true,
                "success": true,
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_delete_build_errors_json() {
        let mock_subgraph_delete = SubgraphDeleteResponse {
            supergraph_was_updated: false,
            build_errors: vec![
                BuildError::composition_error(
                    Some("AN_ERROR_CODE".to_string()),
                    Some("[Accounts] -> Things went really wrong".to_string()),
                ),
                BuildError::composition_error(
                    None,
                    Some("[Films] -> Something else also went wrong".to_string()),
                ),
            ]
            .into(),
        };
        let actual_json: JsonOutput = RoverOutput::SubgraphDeleteResponse {
            delete_response: mock_subgraph_delete,
            subgraph: "subgraph".to_string(),
            dry_run: true,
            graph_ref: GraphRef {
                name: "name".to_string(),
                variant: "current".to_string(),
            },
        }
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "supergraph_was_updated": false,
                "success": true,
            },
            "error": {
                "message": "Encountered 2 build errors while trying to build subgraph \"subgraph\" into supergraph \"name@current\".",
                "code": "E029",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition"
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition"
                        }
                    ],
                }
            }
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn supergraph_fetch_no_successful_publishes_json() {
        let graph_ref = GraphRef {
            name: "name".to_string(),
            variant: "current".to_string(),
        };
        let source = BuildErrors::from(vec![
            BuildError::composition_error(
                Some("AN_ERROR_CODE".to_string()),
                Some("[Accounts] -> Things went really wrong".to_string()),
            ),
            BuildError::composition_error(
                None,
                Some("[Films] -> Something else also went wrong".to_string()),
            ),
        ]);
        let actual_json: JsonOutput =
            RoverError::new(RoverClientError::NoSupergraphBuilds { graph_ref, source }).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": false
            },
            "error": {
                "message": "No supergraph SDL exists for \"name@current\" because its subgraphs failed to build.",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition",
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition"
                        }
                    ]
                },
                "code": "E027"
            }
        });
        assert_json_eq!(actual_json, expected_json);
    }

    #[test]
    fn check_success_response_json() {
        let graph_ref = GraphRef {
            name: "name".to_string(),
            variant: "current".to_string(),
        };
        let mock_check_response = CheckResponse::try_new(
            Some("https://studio.apollographql.com/graph/my-graph/composition/big-hash?variant=current".to_string()),
            10,
            vec![
                SchemaChange {
                    code: "SOMETHING_HAPPENED".to_string(),
                    description: "beeg yoshi".to_string(),
                    severity: ChangeSeverity::PASS,
                },
                SchemaChange {
                    code: "WOW".to_string(),
                    description: "that was so cool".to_string(),
                    severity: ChangeSeverity::PASS,
                }
            ],
            ChangeSeverity::PASS,
            graph_ref,
        );
        if let Ok(mock_check_response) = mock_check_response {
            let actual_json: JsonOutput = RoverOutput::CheckResponse(mock_check_response).into();
            let expected_json = json!(
            {
                "json_version": "1",
                "data": {
                    "target_url": "https://studio.apollographql.com/graph/my-graph/composition/big-hash?variant=current",
                    "operation_check_count": 10,
                    "changes": [
                        {
                            "code": "SOMETHING_HAPPENED",
                            "description": "beeg yoshi",
                            "severity": "PASS"
                        },
                        {
                            "code": "WOW",
                            "description": "that was so cool",
                            "severity": "PASS"
                        },
                    ],
                    "failure_count": 0,
                    "success": true,
                },
                "error": null
            });
            assert_json_eq!(expected_json, actual_json);
        } else {
            panic!("The shape of this response should return a CheckResponse")
        }
    }

    #[test]
    fn check_failure_response_json() {
        let graph_ref = GraphRef {
            name: "name".to_string(),
            variant: "current".to_string(),
        };
        let check_response = CheckResponse::try_new(
            Some("https://studio.apollographql.com/graph/my-graph/composition/big-hash?variant=current".to_string()),
            10,
            vec![
                SchemaChange {
                    code: "SOMETHING_HAPPENED".to_string(),
                    description: "beeg yoshi".to_string(),
                    severity: ChangeSeverity::FAIL,
                },
                SchemaChange {
                    code: "WOW".to_string(),
                    description: "that was so cool".to_string(),
                    severity: ChangeSeverity::FAIL,
                }
            ],
            ChangeSeverity::FAIL, graph_ref);

        if let Err(operation_check_failure) = check_response {
            let actual_json: JsonOutput = RoverError::new(operation_check_failure).into();
            let expected_json = json!(
            {
                "json_version": "1",
                "data": {
                    "target_url": "https://studio.apollographql.com/graph/my-graph/composition/big-hash?variant=current",
                    "operation_check_count": 10,
                    "changes": [
                        {
                            "code": "SOMETHING_HAPPENED",
                            "description": "beeg yoshi",
                            "severity": "FAIL"
                        },
                        {
                            "code": "WOW",
                            "description": "that was so cool",
                            "severity": "FAIL"
                        },
                    ],
                    "failure_count": 2,
                    "success": false,
                },
                "error": {
                    "message": "This operation check has encountered 2 schema changes that would break operations from existing client traffic.",
                    "code": "E030",
                }
            });
            assert_json_eq!(expected_json, actual_json);
        } else {
            panic!("The shape of this response should return a RoverClientError")
        }
    }

    #[test]
    fn graph_publish_response_json() {
        let mock_publish_response = GraphPublishResponse {
            api_schema_hash: "123456".to_string(),
            change_summary: ChangeSummary {
                field_changes: FieldChanges {
                    additions: 2,
                    removals: 1,
                    edits: 0,
                },
                type_changes: TypeChanges {
                    additions: 4,
                    removals: 0,
                    edits: 7,
                },
            },
        };
        let actual_json: JsonOutput = RoverOutput::GraphPublishResponse {
            graph_ref: GraphRef {
                name: "graph".to_string(),
                variant: "variant".to_string(),
            },
            publish_response: mock_publish_response,
        }
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "api_schema_hash": "123456",
                "field_changes": {
                    "additions": 2,
                    "removals": 1,
                    "edits": 0
                },
                "type_changes": {
                    "additions": 4,
                    "removals": 0,
                    "edits": 7
                },
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_publish_success_response_json() {
        let mock_publish_response = SubgraphPublishResponse {
            api_schema_hash: Some("123456".to_string()),
            build_errors: BuildErrors::new(),
            supergraph_was_updated: true,
            subgraph_was_created: true,
        };
        let actual_json: JsonOutput = RoverOutput::SubgraphPublishResponse {
            graph_ref: GraphRef {
                name: "graph".to_string(),
                variant: "variant".to_string(),
            },
            subgraph: "subgraph".to_string(),
            publish_response: mock_publish_response,
        }
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "api_schema_hash": "123456",
                "supergraph_was_updated": true,
                "subgraph_was_created": true,
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_publish_failure_response_json() {
        let mock_publish_response = SubgraphPublishResponse {
            api_schema_hash: None,

            build_errors: vec![
                BuildError::composition_error(
                    Some("AN_ERROR_CODE".to_string()),
                    Some("[Accounts] -> Things went really wrong".to_string()),
                ),
                BuildError::composition_error(
                    None,
                    Some("[Films] -> Something else also went wrong".to_string()),
                ),
            ]
            .into(),
            supergraph_was_updated: false,
            subgraph_was_created: false,
        };
        let actual_json: JsonOutput = RoverOutput::SubgraphPublishResponse {
            graph_ref: GraphRef {
                name: "name".to_string(),
                variant: "current".to_string(),
            },
            subgraph: "subgraph".to_string(),
            publish_response: mock_publish_response,
        }
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "api_schema_hash": null,
                "subgraph_was_created": false,
                "supergraph_was_updated": false,
                "success": true
            },
            "error": {
                "message": "Encountered 2 build errors while trying to build subgraph \"subgraph\" into supergraph \"name@current\".",
                "code": "E029",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition",
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition"
                        }
                    ]
                }
            }
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn profiles_json() {
        let mock_profiles = vec!["default".to_string(), "staging".to_string()];
        let actual_json: JsonOutput = RoverOutput::Profiles(mock_profiles).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "profiles": [
                    "default",
                    "staging"
                ],
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn introspection_json() {
        let actual_json: JsonOutput = RoverOutput::Introspection(
            "i cant believe its not a real introspection response".to_string(),
        )
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "introspection_response": "i cant believe its not a real introspection response",
                "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn error_explanation_json() {
        let actual_json: JsonOutput = RoverOutput::ErrorExplanation(
            "this error occurs when stuff is real complicated... I wouldn't worry about it"
                .to_string(),
        )
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "explanation_markdown": "this error occurs when stuff is real complicated... I wouldn't worry about it",
                "success": true
            },
            "error": null
        }

        );
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn empty_success_json() {
        let actual_json: JsonOutput = RoverOutput::EmptySuccess.into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
               "success": true
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn base_error_message_json() {
        let actual_json: JsonOutput = RoverError::new(anyhow!("Some random error")).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": false
            },
            "error": {
                "message": "Some random error",
                "code": null
            }
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn coded_error_message_json() {
        let actual_json: JsonOutput = RoverError::new(RoverClientError::NoSubgraphInGraph {
            invalid_subgraph: "invalid_subgraph".to_string(),
            valid_subgraphs: Vec::new(),
        })
        .into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": false
            },
            "error": {
                "message": "Could not find subgraph \"invalid_subgraph\".",
                "code": "E009"
            }
        });
        assert_json_eq!(expected_json, actual_json)
    }

    #[test]
    fn composition_error_message_json() {
        let source = BuildErrors::from(vec![
            BuildError::composition_error(
                Some("AN_ERROR_CODE".to_string()),
                Some("[Accounts] -> Things went really wrong".to_string()),
            ),
            BuildError::composition_error(
                None,
                Some("[Films] -> Something else also went wrong".to_string()),
            ),
        ]);
        let actual_json: JsonOutput =
            RoverError::from(RoverClientError::BuildErrors { source }).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": false
            },
            "error": {
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition"
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition"
                        }
                    ],
                },
                "message": "Encountered 2 build errors while trying to build a supergraph.",
                "code": "E029"
            }
        });
        assert_json_eq!(expected_json, actual_json)
    }
}
