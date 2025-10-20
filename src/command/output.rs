#[cfg(feature = "composition-js")]
use crate::command::connector::run::{RunConnector, RunConnectorOutput};
use crate::command::docs::shortlinks::ShortlinkInfo;
use crate::{
    RoverError,
    command::{
        supergraph::compose::CompositionOutput,
        template::queries::list_templates_for_language::ListTemplatesForLanguageTemplates,
    },
    options::{JsonVersion, ProjectLanguage},
    utils::table,
};
use calm_io::{stderr, stderrln};
use camino::Utf8PathBuf;
use comfy_table::{Attribute::Bold, Cell, CellAlignment::Center};
use rover_client::{
    RoverClientError,
    operations::{
        api_key::list::ApiKey,
        contract::{describe::ContractDescribeResponse, publish::ContractPublishResponse},
        graph::publish::GraphPublishResponse,
        init::memberships::InitMembershipsResponse,
        persisted_queries::publish::PersistedQueriesPublishResponse,
        subgraph::{
            delete::SubgraphDeleteResponse, list::SubgraphListResponse,
            publish::SubgraphPublishResponse,
        },
    },
    shared::{
        CheckRequestSuccessResult, CheckWorkflowResponse, FetchResponse, GraphRef, LintResponse,
        SdlType,
    },
};
use rover_std::Style;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fmt::Write,
    io::{self, IsTerminal},
};
use termimad::{MadSkin, crossterm::style::Attribute::Underlined};

/// RoverOutput defines all of the different types of data that are printed
/// to `stdout`. Every one of Rover's commands should return `saucer::Result<RoverOutput>`
/// If the command needs to output some type of data, it should be structured
/// in this enum, and its print logic should be handled in `RoverOutput::get_stdout`
///
/// Not all commands will output machine readable information, and those should
/// return `Ok(RoverOutput::EmptySuccess)`. If a new command is added and it needs to
/// return something that is not described well in this enum, it should be added.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum RoverOutput {
    ConfigWhoAmIOutput {
        api_key: String,
        graph_id: Option<String>,
        graph_title: Option<String>,
        key_type: String,
        origin: String,
        user_id: Option<String>,
    },
    InitMembershipsOutput(InitMembershipsResponse),
    ContractDescribe(ContractDescribeResponse),
    ContractPublish(ContractPublishResponse),
    DocsList(BTreeMap<&'static str, ShortlinkInfo>),
    FetchResponse(FetchResponse),
    SupergraphSchema(String),
    JsonSchema(String),
    CompositionResult(CompositionOutput),
    SubgraphList(SubgraphListResponse),
    CheckWorkflowResponse(CheckWorkflowResponse),
    AsyncCheckResponse(CheckRequestSuccessResult),
    LintResponse(LintResponse),
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
    TemplateList(Vec<ListTemplatesForLanguageTemplates>),
    TemplateUseSuccess {
        template_id: String,
        path: Utf8PathBuf,
    },
    Profiles(Vec<String>),
    Introspection(String),
    ErrorExplanation(String),
    ReadmeFetchResponse {
        graph_ref: GraphRef,
        content: String,
        last_updated_time: Option<String>,
    },
    ReadmePublishResponse {
        graph_ref: GraphRef,
        new_content: String,
        last_updated_time: Option<String>,
    },
    PersistedQueriesPublishResponse(PersistedQueriesPublishResponse),
    LicenseResponse {
        graph_id: String,
        jwt: String,
    },
    EmptySuccess,
    CloudConfigFetchResponse {
        config: String,
    },
    MessageResponse {
        msg: String,
    },
    #[cfg(feature = "composition-js")]
    ConnectorRunResponse {
        output: RunConnectorOutput,
    },
    #[cfg(feature = "composition-js")]
    ConnectorTestResponse {
        output: String,
    },
    CreateKeyResponse {
        api_key: String,
        key_type: String,
        id: String,
        name: String,
    },
    DeleteKeyResponse {
        id: String,
    },
    ListKeysResponse {
        keys: Vec<ApiKey>,
    },
    RenameKeyResponse {
        id: String,
        old_name: Option<String>,
        new_name: String,
    },
}

impl RoverOutput {
    pub fn get_stdout(&self) -> io::Result<Option<String>> {
        Ok(match self {
            RoverOutput::ConfigWhoAmIOutput {
                api_key,
                graph_id,
                graph_title,
                key_type,
                origin,
                user_id,
            } => {
                let mut table = table::get_table();

                table.add_row(vec![&Style::WhoAmIKey.paint("Key Type"), key_type]);

                if let Some(graph_id) = graph_id {
                    table.add_row(vec![&Style::WhoAmIKey.paint("Graph ID"), graph_id]);
                }

                if let Some(graph_title) = graph_title {
                    table.add_row(vec![&Style::WhoAmIKey.paint("Graph Title"), graph_title]);
                }

                if let Some(user_id) = user_id {
                    table.add_row(vec![&Style::WhoAmIKey.paint("User ID"), user_id]);
                }

                table.add_row(vec![&Style::WhoAmIKey.paint("Origin"), origin]);
                table.add_row(vec![&Style::WhoAmIKey.paint("API Key"), api_key]);

                Some(format!("{table}"))
            }
            RoverOutput::InitMembershipsOutput(init_memberships_response) => {
                let mut table = table::get_table();
                table.add_row(vec![
                    &Style::WhoAmIKey.paint("Organization Name"),
                    &Style::WhoAmIKey.paint("Organization ID"),
                ]);
                for o in init_memberships_response.memberships.iter().cycle() {
                    table.add_row(vec![o.name.clone(), o.id.clone()]);
                }

                Some(format!("{table}"))
            }
            RoverOutput::ContractDescribe(describe_response) => Some(format!(
                "{description}\nView the variant's full configuration at {variant_config}",
                description = &describe_response.description,
                variant_config = Style::Link.paint(format!(
                    "{}/graph/{}/settings/variant?variant={}",
                    describe_response.root_url,
                    describe_response.graph_ref.name,
                    describe_response.graph_ref.variant,
                ))
            )),
            RoverOutput::ContractPublish(publish_response) => {
                let launch_cli_copy = publish_response
                    .launch_cli_copy
                    .clone()
                    .unwrap_or_else(|| "No launch was triggered for this publish.".to_string());
                Some(format!(
                    "{description}\n{launch_cli_copy}",
                    description = &publish_response.config_description
                ))
            }
            RoverOutput::DocsList(shortlinks) => {
                stderrln!(
                    "You can open any of these documentation pages by running {}.\n",
                    Style::Command.paint("`rover docs open <slug>`")
                )?;
                let mut table = table::get_table();

                table.set_header(
                    vec!["Slug", "Description"]
                        .into_iter()
                        .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
                );
                for (slug, shortlink_info) in shortlinks {
                    table.add_row(vec![slug, shortlink_info.description]);
                }
                Some(format!("{table}"))
            }
            RoverOutput::FetchResponse(fetch_response) => {
                Some((fetch_response.sdl.contents).to_string())
            }
            RoverOutput::GraphPublishResponse {
                graph_ref,
                publish_response,
            } => {
                stderrln!(
                    "{}#{} published successfully {}",
                    graph_ref,
                    publish_response.api_schema_hash,
                    publish_response.change_summary
                )?;
                Some((publish_response.api_schema_hash).to_string())
            }
            RoverOutput::SubgraphPublishResponse {
                graph_ref,
                subgraph,
                publish_response,
            } => {
                if publish_response.subgraph_was_created {
                    stderrln!(
                        "A new subgraph called '{}' was created in '{}'",
                        subgraph,
                        graph_ref
                    )?;
                } else if publish_response.subgraph_was_updated {
                    stderrln!("The '{}' subgraph in '{}' was updated", subgraph, graph_ref)?;
                } else {
                    stderrln!(
                        "The '{}' subgraph was NOT updated because no changes were detected",
                        subgraph
                    )?;
                }

                if publish_response.supergraph_was_updated {
                    stderrln!(
                        "The supergraph schema for '{}' was updated, composed from the updated '{}' subgraph",
                        graph_ref,
                        subgraph
                    )?;
                } else {
                    stderrln!(
                        "The supergraph schema for '{}' was NOT updated with a new schema",
                        graph_ref
                    )?;
                }

                if let Some(launch_cli_copy) = &publish_response.launch_cli_copy {
                    stderrln!("{}", launch_cli_copy)?;
                }

                if !publish_response.build_errors.is_empty() {
                    let warn_prefix = Style::WarningPrefix.paint("WARN:");
                    stderrln!("{} The following build errors occurred:", warn_prefix)?;
                    stderrln!("{}", &publish_response.build_errors)?;
                }
                None
            }
            RoverOutput::SubgraphDeleteResponse {
                graph_ref,
                subgraph,
                dry_run,
                delete_response,
            } => {
                let warn_prefix = Style::WarningPrefix.paint("WARN:");
                if *dry_run {
                    if !delete_response.build_errors.is_empty() {
                        stderrln!(
                            "{} Deleting the {} subgraph from {} would result in the following build errors:",
                            warn_prefix,
                            Style::Link.paint(subgraph),
                            Style::Link.paint(graph_ref.to_string()),
                        )?;

                        stderrln!("{}", &delete_response.build_errors)?;
                        stderrln!(
                            "{} This is only a prediction. If the graph changes before confirming, these errors could change.",
                            warn_prefix
                        )?;
                    } else {
                        stderrln!(
                            "{} At the time of checking, there would be no build errors resulting from the deletion of this subgraph.",
                            warn_prefix
                        )?;
                        stderrln!(
                            "{} This is only a prediction. If the graph changes before confirming, there could be build errors.",
                            warn_prefix
                        )?;
                    }
                    None
                } else {
                    if delete_response.supergraph_was_updated {
                        stderrln!(
                            "The '{}' subgraph was removed from '{}'. The remaining subgraphs were composed.",
                            Style::Link.paint(subgraph),
                            Style::Link.paint(graph_ref.to_string()),
                        )?;
                    } else {
                        stderrln!(
                            "{} The supergraph schema for '{}' was not updated. See errors below.",
                            warn_prefix,
                            Style::Link.paint(graph_ref.to_string())
                        )?;
                    }

                    if !delete_response.build_errors.is_empty() {
                        stderrln!(
                            "{} There were build errors as a result of deleting the '{}' subgraph from '{}':",
                            warn_prefix,
                            Style::Link.paint(subgraph),
                            Style::Link.paint(graph_ref.to_string())
                        )?;

                        stderrln!("{}", &delete_response.build_errors)?;
                    }
                    None
                }
            }
            RoverOutput::SupergraphSchema(csdl) => Some((csdl).to_string()),
            RoverOutput::JsonSchema(schema) => Some(schema.clone()),
            RoverOutput::CompositionResult(composition_output) => {
                let warn_prefix = Style::HintPrefix.paint("HINT:");

                let hints_string =
                    composition_output
                        .hints
                        .iter()
                        .fold(String::new(), |mut output, hint| {
                            let _ = writeln!(output, "{} {}", warn_prefix, hint.message);
                            output
                        });

                stderrln!("{}", hints_string)?;

                Some((composition_output.supergraph_sdl).to_string())
            }
            RoverOutput::SubgraphList(details) => {
                let mut table = table::get_table();

                table.set_header(
                    vec!["Name", "Routing Url", "Last Updated"]
                        .into_iter()
                        .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
                );

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

                    table.add_row(vec![subgraph.name.clone(), url, formatted_updated_at]);
                }
                Some(format!(
                    "{}\n View full details at {}/graph/{}/service-list",
                    table, details.root_url, details.graph_ref.name
                ))
            }
            RoverOutput::TemplateList(templates) => {
                let mut table = table::get_table();

                table.set_header(
                    vec!["Name", "ID", "Language", "Repo URL"]
                        .into_iter()
                        .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
                );

                for template in templates {
                    let language: ProjectLanguage = template.language.clone().into();
                    table.add_row(vec![
                        template.name.clone(),
                        template.id.clone(),
                        language.to_string(),
                        template.repo_url.to_string(),
                    ]);
                }

                Some(format!("{table}"))
            }
            RoverOutput::TemplateUseSuccess { template_id, path } => {
                let template_id = Style::Command.paint(template_id);
                let path = Style::Path.paint(path.as_str());
                let readme = Style::Path.paint("README.md");
                let forum_call_to_action = Style::CallToAction.paint(
                    "Have a question or suggestion about templates? Let us know at \
                    https://community.apollographql.com/",
                );
                Some(format!(
                    "Successfully created a new project from the '{template_id}' template in {path}\n Read the generated '{readme}' file for next steps.\n{forum_call_to_action}"
                ))
            }
            RoverOutput::CheckWorkflowResponse(check_response) => Some(check_response.get_output()),
            RoverOutput::AsyncCheckResponse(check_response) => Some(format!(
                "Check successfully started with workflow ID: {}\nView full details at {}",
                check_response.workflow_id, check_response.target_url
            )),
            RoverOutput::LintResponse(lint_response) => Some(lint_response.get_ariadne()?),
            RoverOutput::Profiles(profiles) => {
                if profiles.is_empty() {
                    stderrln!("No profiles found.")?;
                }
                Some(profiles.join("\n"))
            }
            RoverOutput::Introspection(introspection_response) => {
                Some((introspection_response).to_string())
            }
            RoverOutput::ErrorExplanation(explanation) => {
                // underline bolded md
                let mut skin = MadSkin::default();
                skin.bold.add_attr(Underlined);

                Some(format!("{}", skin.inline(explanation)))
            }
            RoverOutput::ReadmeFetchResponse {
                graph_ref: _,
                content,
                last_updated_time: _,
            } => Some((content).to_string()),
            RoverOutput::ReadmePublishResponse {
                graph_ref,
                new_content: _,
                last_updated_time: _,
            } => {
                stderrln!("Readme for {} published successfully", graph_ref,)?;
                None
            }
            RoverOutput::PersistedQueriesPublishResponse(response) => {
                let result = if response.unchanged {
                    format!(
                        "Successfully published {} operations, resulting in no changes to {}, which contains {} operations.",
                        Style::NewOperationCount
                            .paint(response.total_published_operations.to_string()),
                        Style::PersistedQueryList.paint(&response.list_name),
                        Style::TotalOperationCount
                            .paint(response.operation_counts.total().to_string())
                    )
                } else {
                    let mut result = "Successfully ".to_string();

                    result.push_str(&match (
                            response.operation_counts.added_str().map(|s| Style::Command.paint(s)),
                            response.operation_counts.updated_str().map(|s| Style::Command.paint(s)),
                            response.operation_counts.removed_str().map(|s| Style::Command.paint(s)),
                        ) {
                            (Some(added), Some(updated), Some(removed)) => format!(
                                "added {added}, updated {updated}, and removed {removed}, creating"
                            ),
                            (Some(added), Some(updated), None) => {
                                format!("added {added} and updated {updated}, creating")
                            }
                            (Some(added), None, Some(removed)) => {
                                format!("added {added} and removed {removed}, creating")
                            }
                            (None, Some(updated), Some(removed)) => {
                                format!("updated {updated} and removed {removed}, creating")
                            }
                            (Some(added), None, None) => {
                                format!("added {added}, creating")
                            }
                            (None, None, Some(removed)) => {
                                format!("removed {removed}, creating")
                            }
                            (None, Some(updated), None) => {
                                format!("updated {updated}, creating")
                            }
                            (None, None, None) => unreachable!("persisted query list {} claimed there were changes (unchanged != null), but added, removed, and updated were all 0", response.list_id),
                        });

                    result.push_str(&format!(
                        " revision {} of {}, which contains {} operations.",
                        Style::Command.paint(response.revision.to_string()),
                        Style::Command.paint(&response.list_name),
                        Style::Command.paint(response.operation_counts.total().to_string())
                    ));

                    result
                };

                Some(result)
            }
            RoverOutput::LicenseResponse { jwt, .. } => {
                stderrln!("Success!")?;
                Some(jwt.to_string())
            }
            RoverOutput::EmptySuccess => None,
            RoverOutput::CloudConfigFetchResponse { config } => Some(config.to_string()),
            RoverOutput::MessageResponse { msg } => Some(msg.into()),
            #[cfg(feature = "composition-js")]
            RoverOutput::ConnectorRunResponse { output } => {
                Some(RunConnector::format_output(output))
            }
            #[cfg(feature = "composition-js")]
            RoverOutput::ConnectorTestResponse { output } => Some(output.into()),
            RoverOutput::CreateKeyResponse {
                api_key,
                key_type,
                id,
                name,
            } => {
                let mut table = table::get_table();

                table.add_row(vec![&Style::WhoAmIKey.paint("ID"), id]);
                table.add_row(vec![&Style::WhoAmIKey.paint("Name"), name]);
                table.add_row(vec![&Style::WhoAmIKey.paint("Key Type"), key_type]);
                table.add_row(vec![&Style::WhoAmIKey.paint("API Key"), api_key]);

                Some(format!("{table}"))
            }
            RoverOutput::DeleteKeyResponse { id } => {
                stderrln!("Deleted API Key {id}")?;
                None
            }
            RoverOutput::ListKeysResponse { keys } => {
                let mut table = table::get_table();

                table.set_header(vec!["ID", "Name", "Created At", "Expires At"]);
                for key in keys {
                    table.add_row(vec![
                        key.id.clone(),
                        key.name.clone().unwrap_or(String::new()),
                        key.created_at.to_string(),
                        key.expires_at
                            .map(|timestamp| timestamp.to_string())
                            .unwrap_or_else(|| "Never".to_string()),
                    ]);
                }
                Some(format!("{table}"))
            }
            RoverOutput::RenameKeyResponse {
                id,
                old_name,
                new_name,
            } => {
                let display_old_name = old_name.clone().unwrap_or(String::new());
                stderrln!("Renamed API Key {id} from '{display_old_name}' to '{new_name}'")?;
                None
            }
        })
    }

    pub(crate) fn get_internal_data_json(&self) -> Value {
        match self {
            RoverOutput::ConfigWhoAmIOutput {
                key_type,
                origin,
                api_key,
                graph_title,
                graph_id,
                user_id,
            } => {
                json!({
                  "key_type": key_type,
                  "graph_id": graph_id,
                  "graph_title": graph_title,
                  "user_id": user_id,
                  "origin": origin,
                  "api_key": api_key,
                })
            }
            RoverOutput::InitMembershipsOutput(memberships_response) => json!(memberships_response),
            RoverOutput::ContractDescribe(describe_response) => json!(describe_response),
            RoverOutput::ContractPublish(publish_response) => json!(publish_response),
            RoverOutput::DocsList(shortlinks) => {
                let mut shortlink_vec = Vec::with_capacity(shortlinks.len());
                for (shortlink_slug, shortlink_info) in shortlinks {
                    shortlink_vec.push(
                        json!({"slug": shortlink_slug, "description": shortlink_info.description }),
                    );
                }
                json!({ "shortlinks": shortlink_vec })
            }
            RoverOutput::FetchResponse(fetch_response) => json!(fetch_response),
            RoverOutput::SupergraphSchema(csdl) => json!({ "core_schema": csdl }),
            RoverOutput::JsonSchema(schema) => Value::String(schema.clone()),
            RoverOutput::CompositionResult(composition_output) => {
                if let Some(federation_version) = &composition_output.federation_version {
                    json!({
                      "core_schema": composition_output.supergraph_sdl,
                      "hints": composition_output.hints,
                      "federation_version": federation_version
                    })
                } else {
                    json!({
                        "core_schema": composition_output.supergraph_sdl,
                        "hints": composition_output.hints
                    })
                }
            }
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
            RoverOutput::TemplateList(templates) => json!({ "templates": templates }),
            RoverOutput::TemplateUseSuccess { template_id, path } => {
                json!({ "template_id": template_id, "path": path })
            }
            RoverOutput::CheckWorkflowResponse(check_response) => check_response.get_json(),
            RoverOutput::AsyncCheckResponse(check_response) => check_response.get_json(),
            RoverOutput::LintResponse(lint_response) => lint_response.get_json(),
            RoverOutput::Profiles(profiles) => json!({ "profiles": profiles }),
            RoverOutput::Introspection(introspection_response) => {
                json!({ "introspection_response": introspection_response })
            }
            RoverOutput::ErrorExplanation(explanation_markdown) => {
                json!({ "explanation_markdown": explanation_markdown })
            }
            RoverOutput::ReadmeFetchResponse {
                graph_ref: _,
                content,
                last_updated_time,
            } => {
                json!({ "readme": content, "last_updated_time": last_updated_time})
            }
            RoverOutput::ReadmePublishResponse {
                graph_ref: _,
                new_content,
                last_updated_time,
            } => {
                json!({ "readme": new_content, "last_updated_time": last_updated_time })
            }
            RoverOutput::EmptySuccess => json!(null),
            RoverOutput::PersistedQueriesPublishResponse(response) => {
                json!({
                  "revision": response.revision,
                  "list": {
                      "id": response.list_id,
                      "name": response.list_name
                  },
                  "unchanged": response.unchanged,
                  "operation_counts": {
                    "added": response.operation_counts.added,
                    "identical": response.operation_counts.identical,
                    "total": response.operation_counts.total(),
                    "unaffected": response.operation_counts.unaffected,
                    "updated": response.operation_counts.updated,
                    "removed": response.operation_counts.removed,
                  },
                  "total_published_operations": response.total_published_operations,
                })
            }
            RoverOutput::LicenseResponse { jwt, .. } => {
                json!({"jwt": jwt })
            }
            RoverOutput::CloudConfigFetchResponse { config } => {
                json!({ "config": config })
            }
            RoverOutput::MessageResponse { msg } => {
                json!({ "message": msg })
            }
            #[cfg(feature = "composition-js")]
            RoverOutput::ConnectorRunResponse { output } => {
                json!({ "output": output })
            }
            #[cfg(feature = "composition-js")]
            RoverOutput::ConnectorTestResponse { output } => json!({ "output": output }),
            RoverOutput::CreateKeyResponse {
                api_key,
                key_type,
                id,
                name,
            } => {
                json!({ "api_key": api_key, "key_type": key_type, "id": id, "name": name })
            }
            RoverOutput::DeleteKeyResponse { id } => {
                json!({ "id": id })
            }
            RoverOutput::ListKeysResponse { keys } => {
                json!({ "keys": keys })
            }
            RoverOutput::RenameKeyResponse {
                id,
                old_name,
                new_name,
            } => {
                json!({ "old_name": old_name, "new_name": new_name, "id": id })
            }
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
            Self::CheckWorkflowResponse(_) => JsonVersion::Two,
            _ => JsonVersion::default(),
        }
    }

    pub(crate) fn print_descriptor(&self) -> io::Result<()> {
        if std::io::stdout().is_terminal()
            && let Some(descriptor) = self.descriptor()
        {
            stderrln!("{}: \n", Style::Heading.paint(descriptor))?;
        }
        Ok(())
    }
    pub(crate) fn print_one_line_descriptor(&self) -> io::Result<()> {
        if std::io::stdout().is_terminal()
            && let Some(descriptor) = self.descriptor()
        {
            stderr!("{}: ", Style::Heading.paint(descriptor))?;
        }
        Ok(())
    }
    pub(crate) const fn descriptor(&self) -> Option<&str> {
        match &self {
            RoverOutput::ContractDescribe(_) => Some("Configuration Description"),
            RoverOutput::ContractPublish(_) => Some("New Configuration Description"),
            RoverOutput::FetchResponse(fetch_response) => match fetch_response.sdl.r#type {
                SdlType::Graph | SdlType::Subgraph { .. } => Some("Schema"),
                SdlType::Supergraph => Some("Supergraph Schema"),
            },
            RoverOutput::CompositionResult(_) | RoverOutput::SupergraphSchema(_) => {
                Some("Supergraph Schema")
            }
            RoverOutput::TemplateUseSuccess { .. } => Some("Project generated"),
            RoverOutput::AsyncCheckResponse(_) => Some("Check Started"),
            RoverOutput::Profiles(_) => Some("Profiles"),
            RoverOutput::Introspection(_) => Some("Introspection Response"),
            RoverOutput::ReadmeFetchResponse { .. } => Some("Readme"),
            RoverOutput::GraphPublishResponse { .. } => Some("Schema Hash"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::JsonOutput;
    use anyhow::anyhow;
    use apollo_federation_types::rover::{BuildError, BuildErrors};
    use assert_json_diff::assert_json_eq;
    use chrono::{DateTime, Local, Utc};
    use console::strip_ansi_codes;
    use rover_client::{
        operations::{
            graph::publish::{ChangeSummary, FieldChanges, TypeChanges},
            persisted_queries::publish::PersistedQueriesOperationCounts,
            subgraph::{
                delete::SubgraphDeleteResponse,
                list::{SubgraphInfo, SubgraphUpdatedAt},
            },
        },
        shared::{
            ChangeSeverity, CheckTaskStatus, CheckWorkflowResponse, CustomCheckResponse,
            Diagnostic, LintCheckResponse, OperationCheckResponse, ProposalsCheckResponse,
            ProposalsCheckSeverityLevel, ProposalsCoverage, RelatedProposal, SchemaChange, Sdl,
            SdlType, Violation,
        },
    };
    use std::collections::BTreeMap;

    #[test]
    fn docs_list_json() {
        let mut mock_shortlinks = BTreeMap::new();
        mock_shortlinks.insert(
            "slug_one",
            ShortlinkInfo::new("description_one", "r", "slug_one"),
        );
        mock_shortlinks.insert(
            "slug_two",
            ShortlinkInfo::new("description_two", "r", "slug_two"),
        );
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
        let actual_json: JsonOutput = RoverOutput::SupergraphSchema(mock_core_schema).into();
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
                    None,
                    None,
                ),
                BuildError::composition_error(
                    None,
                    Some("[Films] -> Something else also went wrong".to_string()),
                    None,
                    None,
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
                "message": "Encountered 2 build errors while trying to build subgraph 'subgraph' into supergraph 'name@current'.",
                "code": "E029",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
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
                None,
                None,
            ),
            BuildError::composition_error(
                None,
                Some("[Films] -> Something else also went wrong".to_string()),
                None,
                None,
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
                "message": "No supergraph SDL exists for 'name@current' because its subgraphs failed to build.",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
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
        let mock_check_response = CheckWorkflowResponse {
            default_target_url: "https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1".to_string(),
            maybe_core_schema_modified: Some(true),
            maybe_operations_response: Some(OperationCheckResponse::try_new(
                CheckTaskStatus::PASSED,
                Some("https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1".to_string()),
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
                    },
                ],
            )),
            maybe_lint_response: Some(LintCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some("https://studio.apollographql.com/graph/my-graph/variant/current/lint/1".to_string()),
                diagnostics: vec![
                    Diagnostic {
                        rule: "FIELD_NAMES_SHOULD_BE_CAMEL_CASE".to_string(),
                        level: "WARNING".to_string(),
                        message: "Field must be camelCase.".to_string(),
                        coordinate: "Query.all_users".to_string(),
                        start_line: 1,
                        start_byte_offset: 4,
                        end_byte_offset:2,
                    },
                ],
                errors_count: 0,
                warnings_count: 1,
            }),
            maybe_proposals_response: Some(ProposalsCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some("https://studio.apollographql.com/graph/my-graph/variant/current/proposals/1".to_string()),
                severity_level: ProposalsCheckSeverityLevel::WARN,
                proposal_coverage: ProposalsCoverage::NONE,
                related_proposals: vec![RelatedProposal {
                    status: "OPEN".to_string(),
                    display_name: "Mock Proposal".to_string(),
                }],
            }),
            maybe_custom_response: Some(CustomCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some("https://studio.apollographql.com/graph/my-graph/variant/current/custom/1".to_string()),
                violations:  vec![
                    Violation {
                        rule: "NAMING_CONVENTION".to_string(),
                        level: "WARNING".to_string(),
                        message: "Fields must use camelCase.".to_string(),
                        start_line: Some(1),
                    },
                ],
            }),
            maybe_downstream_response: None,
        };

        let actual_json: JsonOutput =
            RoverOutput::CheckWorkflowResponse(mock_check_response).into();
        let expected_json = json!(
        {
            "json_version": "2",
            "data": {
                "success": true,
                "core_schema_modified": true,
                "tasks": {
                    "custom": {
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/custom/1",
                        "task_status": "PASSED",
                        "violations": [
                            {
                                "level": "WARNING",
                                "message": "Fields must use camelCase.",
                                "rule": "NAMING_CONVENTION",
                                "start_line": 1
                            },
                        ],
                    },
                    "operations": {
                        "task_status": "PASSED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1",
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
                    },
                    "lint": {
                        "task_status": "PASSED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/lint/1",
                        "diagnostics": [
                            {
                                "level": "WARNING",
                                "message": "Field must be camelCase.",
                                "coordinate": "Query.all_users",
                                "start_line": 1,
                                "start_byte_offset": 4,
                                "end_byte_offset": 2,
                                "rule": "FIELD_NAMES_SHOULD_BE_CAMEL_CASE"
                            }
                        ],
                        "errors_count": 0,
                        "warnings_count": 1
                    },
                    "proposals": {
                        "proposal_coverage": "NONE",
                        "related_proposals": [
                            {
                                "status": "OPEN",
                                "display_name": "Mock Proposal",
                            }
                        ],
                        "severity_level": "WARN",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/proposals/1",
                        "task_status": "PASSED",
                      }
                }
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn check_success_response_with_empty_lint_and_custom_violations_text() {
        let mock_check_response = CheckWorkflowResponse {
            default_target_url:
                "https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1"
                    .to_string(),
            maybe_core_schema_modified: Some(true),
            maybe_operations_response: None,
            maybe_lint_response: Some(LintCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/variant/current/lint/1"
                        .to_string(),
                ),
                diagnostics: vec![],
                errors_count: 0,
                warnings_count: 0,
            }),
            maybe_proposals_response: None,
            maybe_custom_response: Some(CustomCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/variant/current/custom/1"
                        .to_string(),
                ),
                violations: vec![],
            }),
            maybe_downstream_response: None,
        };

        let actual_text = RoverOutput::CheckWorkflowResponse(mock_check_response)
            .get_stdout()
            .expect("Expected response to be Ok")
            .expect("Expected response to exist");
        let actual_text = strip_ansi_codes(&actual_text);

        let expected_text = "
There were no changes detected in the composed API schema, but the core schema was modified.

Linter Check [PASSED]:
No linting errors or warnings found.
View linter check details at: https://studio.apollographql.com/graph/my-graph/variant/current/lint/1

Custom Check [PASSED]:
No custom check violations found.
View custom check details at: https://studio.apollographql.com/graph/my-graph/variant/current/custom/1";

        assert_eq!(actual_text, expected_text);
    }

    #[test]
    fn check_failure_response_json() {
        let graph_ref = GraphRef {
            name: "name".to_string(),
            variant: "current".to_string(),
        };
        let check_response = CheckWorkflowResponse {
            default_target_url:
                "https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1".to_string(),
            maybe_core_schema_modified: Some(false),
            maybe_operations_response: Some(OperationCheckResponse::try_new(
                CheckTaskStatus::FAILED,
                Some("https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1".to_string()),
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
                    },
                ],
            )),
            maybe_lint_response: Some(LintCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/variant/current/lint/1"
                        .to_string(),
                ),
                diagnostics: vec![
                    Diagnostic {
                        rule: "FIELD_NAMES_SHOULD_BE_CAMEL_CASE".to_string(),
                        level: "WARNING".to_string(),
                        message: "Field must be camelCase.".to_string(),
                        coordinate: "Query.all_users".to_string(),
                        start_line: 2,
                        start_byte_offset: 8,
                        end_byte_offset: 0
                    },
                    Diagnostic {
                        rule: "TYPE_NAMES_SHOULD_BE_PASCAL_CASE".to_string(),
                        level: "ERROR".to_string(),
                        message: "Type name must be PascalCase.".to_string(),
                        coordinate: "userContext".to_string(),
                        start_line: 3,
                        start_byte_offset:5,
                        end_byte_offset: 0,
                    },
                ],
                errors_count: 1,
                warnings_count: 1,
            }),
            maybe_proposals_response: Some(ProposalsCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                target_url: Some("https://studio.apollographql.com/graph/my-graph/variant/current/proposals/1".to_string()),
                severity_level: ProposalsCheckSeverityLevel::ERROR,
                proposal_coverage: ProposalsCoverage::PARTIAL,
                related_proposals: vec![RelatedProposal {
                    status: "OPEN".to_string(),
                    display_name: "Mock Proposal".to_string(),
                }],
            }),
            maybe_custom_response: Some(CustomCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                target_url: Some("https://studio.apollographql.com/graph/my-graph/variant/current/custom/1".to_string()),
                violations:  vec![
                    Violation {
                        rule: "NAMING_CONVENTION".to_string(),
                        level: "ERROR".to_string(),
                        message: "Fields must use camelCase.".to_string(),
                        start_line: Some(2),
                    },
                ],
            }),
            maybe_downstream_response: None,
        };

        let actual_json: JsonOutput = RoverError::new(RoverClientError::CheckWorkflowFailure {
            graph_ref,
            check_response: Box::new(check_response),
        })
        .into();
        let expected_json = json!(
        {
            "json_version": "2",
            "data": {
                "success": false,
                "core_schema_modified": false,
                "tasks": {
                    "custom": {
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/custom/1",
                        "task_status": "FAILED",
                        "violations": [
                            {
                                "level": "ERROR",
                                "message": "Fields must use camelCase.",
                                "rule": "NAMING_CONVENTION",
                                "start_line": 2
                            },
                        ],
                    },
                    "operations": {
                        "task_status": "FAILED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/operationsCheck/1",
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
                    },
                    "lint": {
                        "task_status": "FAILED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/lint/1",
                        "diagnostics": [
                            {
                                "level": "WARNING",
                                "message": "Field must be camelCase.",
                                "coordinate": "Query.all_users",
                                "start_line": 2,
                                "start_byte_offset": 8,
                                "end_byte_offset": 0,
                                "rule": "FIELD_NAMES_SHOULD_BE_CAMEL_CASE"
                            },
                            {
                                "level": "ERROR",
                                "message": "Type name must be PascalCase.",
                                "coordinate": "userContext",
                                "start_line": 3,
                                "start_byte_offset": 5,
                                "end_byte_offset": 0,
                                "rule": "TYPE_NAMES_SHOULD_BE_PASCAL_CASE"
                            }
                        ],
                        "errors_count": 1,
                        "warnings_count": 1
                    },
                    "proposals": {
                        "proposal_coverage": "PARTIAL",
                        "related_proposals": [
                            {
                                "status": "OPEN",
                                "display_name": "Mock Proposal",
                            }
                        ],
                        "severity_level": "ERROR",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/variant/current/proposals/1",
                        "task_status": "FAILED",
                      }
                },
            },
            "error": {
                "message": "The changes in the schema you proposed caused operation, linter, proposal and custom checks to fail.",
                "code": "E043",
            }
        });
        assert_json_eq!(expected_json, actual_json);
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
            subgraph_was_updated: true,
            launch_url: Some("test.com/launchurl".to_string()),
            launch_cli_copy: Some(
                "You can monitor this launch in Apollo Studio: test.com/launchurl".to_string(),
            ),
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
                "subgraph_was_updated": true,
                "success": true,
                "launch_url": "test.com/launchurl",
                "launch_cli_copy": "You can monitor this launch in Apollo Studio: test.com/launchurl",
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
                    None,
                    None,
                ),
                BuildError::composition_error(
                    None,
                    Some("[Films] -> Something else also went wrong".to_string()),
                    None,
                    None,
                ),
            ]
            .into(),
            supergraph_was_updated: false,
            subgraph_was_created: false,
            subgraph_was_updated: true,
            launch_url: None,
            launch_cli_copy: None,
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
                "subgraph_was_updated": true,
                "supergraph_was_updated": false,
                "success": true,
                "launch_url": null,
                "launch_cli_copy": null,
            },
            "error": {
                "message": "Encountered 2 build errors while trying to build subgraph 'subgraph' into supergraph 'name@current'.",
                "code": "E029",
                "details": {
                    "build_errors": [
                        {
                            "message": "[Accounts] -> Things went really wrong",
                            "code": "AN_ERROR_CODE",
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        }
                    ]
                }
            }
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn subgraph_publish_unchanged_response_json() {
        let mock_publish_response = SubgraphPublishResponse {
            api_schema_hash: Some("123456".to_string()),
            build_errors: BuildErrors::new(),
            supergraph_was_updated: false,
            subgraph_was_created: false,
            subgraph_was_updated: false,
            launch_url: None,
            launch_cli_copy: None,
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
                "supergraph_was_updated": false,
                "subgraph_was_created": false,
                "subgraph_was_updated": false,
                "success": true,
                "launch_url": null,
                "launch_cli_copy": null,
            },
            "error": null
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
                "message": "Could not find subgraph 'invalid_subgraph'.",
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
                None,
                None,
            ),
            BuildError::composition_error(
                None,
                Some("[Films] -> Something else also went wrong".to_string()),
                None,
                None,
            ),
        ]);
        let actual_json: JsonOutput = RoverError::from(RoverClientError::BuildErrors {
            source,
            num_subgraphs: 2,
        })
        .into();
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
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        },
                        {
                            "message": "[Films] -> Something else also went wrong",
                            "code": null,
                            "type": "composition",
                            "nodes": null,
                            "omittedNodesCount": null
                        }
                    ],
                },
                "message": "Encountered 2 build errors while trying to build a supergraph.",
                "code": "E029"
            }
        });
        assert_json_eq!(expected_json, actual_json)
    }

    #[test]
    fn lint_response_json() {
        let actual_json: JsonOutput = RoverError::new(RoverClientError::LintFailures {
            lint_response: LintResponse {
                diagnostics: [Diagnostic {
                    rule: "FIELD_NAMES_SHOULD_BE_CAMEL_CASE".to_string(),
                    coordinate: "Query.Hello".to_string(),
                    level: "ERROR".to_string(),
                    message: "Field names should use camelCase style.".to_string(),
                    start_line: 0,
                    start_byte_offset: 13,
                    end_byte_offset: 18,
                }]
                .to_vec(),
                file_name: "/tmp/schema.graphql".to_string(),
                proposed_schema: "type Query { Hello: String }".to_string(),
            },
        })
        .into();

        let expected_json = json!(
            {
                "data": {
                  "diagnostics": [
                    {
                      "coordinate": "Query.Hello",
                      "level": "ERROR",
                      "message": "Field names should use camelCase style.",
                      "start_line": 0,
                      "start_byte_offset": 13,
                      "end_byte_offset": 18,
                      "rule": "FIELD_NAMES_SHOULD_BE_CAMEL_CASE"
                    }
                  ],
                  "file_name": "/tmp/schema.graphql",
                  "proposed_schema": "type Query { Hello: String }",
                  "success": false
                },
                "error": {
                  "code": "E042",
                  "message": "While linting the proposed schema, some rule violations were found"
                },
                "json_version": "1"
              }
        );
        assert_json_eq!(expected_json, actual_json)
    }

    #[test]
    fn pq_publish_unchanged_response_json() {
        let revision = 1;
        let list_id = "list_id".to_string();
        let graph_id = "graph_id".to_string();
        let list_name = "my list".to_string();
        let total_published_operations = 10;
        let added = 5;
        let identical = 3;
        let removed = 0;
        let unaffected = 2;
        let updated = 2;
        let total = added + identical - removed + unaffected + updated;
        let operation_counts = PersistedQueriesOperationCounts {
            added,
            identical,
            removed,
            unaffected,
            updated,
        };
        let mock_publish_response = PersistedQueriesPublishResponse {
            unchanged: true,
            graph_id,
            list_id: list_id.clone(),
            list_name: list_name.clone(),
            total_published_operations,
            revision,
            operation_counts,
        };
        let actual_json: JsonOutput =
            RoverOutput::PersistedQueriesPublishResponse(mock_publish_response).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": true,
                "unchanged": true,
                "operation_counts": {
                    "added": added,
                    "removed": removed,
                    "updated": updated,
                    "unaffected": unaffected,
                    "identical": identical,
                    "total": total,
                },
                "list": {
                    "id": list_id,
                    "name": list_name
                },
                "revision": revision,
                "total_published_operations": total_published_operations,
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn pq_publish_new_revision_response_json() {
        let revision = 2;
        let list_id = "list_id".to_string();
        let graph_id = "graph_id".to_string();
        let list_name = "my list".to_string();
        let total_published_operations = 10;
        let added = 5;
        let identical = 3;
        let removed = 0;
        let unaffected = 2;
        let updated = 2;
        let total = added + identical - removed + unaffected + updated;
        let operation_counts = PersistedQueriesOperationCounts {
            added,
            identical,
            removed,
            unaffected,
            updated,
        };
        let mock_publish_response = PersistedQueriesPublishResponse {
            revision,
            graph_id,
            list_id: list_id.clone(),
            list_name: list_name.clone(),
            total_published_operations,
            unchanged: false,
            operation_counts,
        };
        let actual_json: JsonOutput =
            RoverOutput::PersistedQueriesPublishResponse(mock_publish_response).into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "success": true,
                "list": {
                    "id": list_id,
                    "name": list_name
                },
                "unchanged": false,
                "operation_counts": {
                    "added": added,
                    "removed": removed,
                    "updated": updated,
                    "unaffected": unaffected,
                    "identical": identical,
                    "total": total,
                },
                "revision": revision,
                "total_published_operations": total_published_operations,
            },
            "error": null
        });
        assert_json_eq!(expected_json, actual_json);
    }

    #[test]
    fn test_license_response_json() {
        let license_response = RoverOutput::LicenseResponse {
            graph_id: "graph".to_string(),
            jwt: "jwt_token".to_string(),
        };

        let actual_json: JsonOutput = license_response.into();
        let expected_json = json!(
        {
            "json_version": "1",
            "data": {
                "jwt": "jwt_token",
                "success": true
            },
            "error": null
        });

        assert_json_eq!(actual_json, expected_json);
    }
}
