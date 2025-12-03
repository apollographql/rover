use anyhow::anyhow;
use apollo_parser::{Parser, cst};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser as ClapParser;
use serde::Serialize;
use std::collections::HashMap;

use crate::{
    RoverError, RoverOutput, RoverResult,
    client::{
        discovery::{DiscoveryOptions, discover_files},
        extensions::{ExtensionFailure, ExtensionSnippet, validate_extensions},
    },
    options::{OptionalGraphRefOpt, ProfileOpt},
    rover_client::operations::graph::validate_operations,
    rover_client::operations::graph::validate_operations::{
        OperationDocument, ValidateOperationsInput,
    },
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, ClapParser)]
pub struct Check {
    /// Graph reference to validate against (optional; defaults to env/config)
    #[clap(flatten)]
    #[serde(skip_serializing)]
    graph: OptionalGraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Paths (dirs or files) to include.
    #[arg(long = "include", value_name = "PATH", action = clap::ArgAction::Append)]
    include: Vec<Utf8PathBuf>,

    /// Paths to exclude.
    #[arg(long = "exclude", value_name = "PATH", action = clap::ArgAction::Append)]
    exclude: Vec<Utf8PathBuf>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ClientCheckFailure {
    pub file: Utf8PathBuf,
    pub message: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ClientCheckSummary {
    pub graph_ref: Option<String>,
    pub files_scanned: usize,
    pub operations_sent: usize,
    pub failures: Vec<ClientCheckFailure>,
    pub validation_results: Vec<ClientValidationResult>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ClientValidationResult {
    pub operation_name: String,
    pub r#type: String,
    pub code: Option<String>,
    pub description: String,
    pub file: Option<Utf8PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone)]
struct OperationInput {
    name: String,
    body: String,
    file: Utf8PathBuf,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone)]
struct ParsedFile {
    operations: Vec<OperationInput>,
    extensions: Vec<ExtensionSnippet>,
}

impl Check {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: rover_client::shared::GitContext,
        _format: crate::cli::RoverOutputFormatKind,
    ) -> RoverResult<RoverOutput> {
        let root = std::env::current_dir()?;
        let root = Utf8PathBuf::from_path_buf(root)
            .map_err(|_| RoverError::new(anyhow!("current directory is not utf-8")))?;

        let options = DiscoveryOptions {
            includes: self.include.clone(),
            excludes: self.exclude.clone(),
            ..Default::default()
        };

        let files = discover_files(&options, &root, |p| p.extension() == Some("graphql"))?;

        let mut failures = Vec::new();
        let mut operations = Vec::new();
        let mut parse_failures = Vec::new();
        let mut extensions = Vec::new();

        for file in files {
            match rover_std::Fs::read_file(&file) {
                Ok(contents) => match extract_operations(&file, &contents) {
                    Ok(parsed) => {
                        operations.extend(parsed.operations);
                        extensions.extend(parsed.extensions);
                    }
                    Err(msg) => parse_failures.push(ClientCheckFailure { file, message: msg }),
                },
                Err(err) => parse_failures.push(ClientCheckFailure {
                    file,
                    message: err.to_string(),
                }),
            }
        }

        if !parse_failures.is_empty() {
            let detail = parse_failures
                .iter()
                .map(|f| format!("{}: {}", f.file, f.message))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(RoverError::new(anyhow!(format!(
                "Failed to parse {} .graphql file(s):\n{}",
                parse_failures.len(),
                detail
            ))));
        }

        if operations.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No .graphql operations found under the provided includes"
            )));
        }

        let graph_ref =
            self.graph.graph_ref.clone().ok_or_else(|| {
                RoverError::new(anyhow!("A graph ref is required for client check."))
            })?;

        let extension_failures =
            collect_extension_failures(&client_config, &self.profile, &graph_ref, &extensions)
                .await;
        failures.extend(extension_failures);

        let client = client_config.get_authenticated_client(&self.profile)?;

        let validation_results =
            validate_against_remote(&client, &graph_ref, &operations, &git_context).await?;

        let op_lookup: HashMap<_, _> = operations.iter().map(|op| (op.name.clone(), op)).collect();

        let mapped_results = validation_results
            .into_iter()
            .map(|mut res| {
                if let Some(op) = op_lookup.get(&res.operation_name) {
                    res.file = Some(op.file.clone());
                    res.line = Some(op.line);
                    res.column = Some(op.column);
                }
                res
            })
            .collect::<Vec<_>>();

        let summary = ClientCheckSummary {
            graph_ref: Some(graph_ref.to_string()),
            files_scanned: operations.len(),
            operations_sent: operations.len(),
            failures,
            validation_results: mapped_results,
        };

        // Fail if any remote validation result is a failure/invalid or there are local failures.
        let has_errors = summary
            .validation_results
            .iter()
            .any(|r| matches!(r.r#type.as_str(), "FAILURE" | "INVALID"))
            || !summary.failures.is_empty();

        if has_errors {
            Err(RoverError::new(anyhow!(
                "Client check failed for one or more operations"
            )))
        } else {
            Ok(RoverOutput::ClientCheckResponse { summary })
        }
    }
}

fn extract_operations(file: &Utf8Path, contents: &str) -> Result<ParsedFile, String> {
    let parser = Parser::new(contents);
    let tree = parser.parse();
    let errors: Vec<_> = tree.errors().collect();
    if !errors.is_empty() {
        let msg = errors
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(msg);
    }

    let doc = tree.document();
    let mut extensions = Vec::new();
    let mut fragment_texts = Vec::new();
    for definition in doc.definitions() {
        if let cst::Definition::FragmentDefinition(fragment) = definition {
            let range = fragment.syntax().text_range();
            let start: usize = range.start().into();
            let end: usize = range.end().into();
            if let Some(text) = contents.get(start..end) {
                fragment_texts.push(text.to_string());
            }
        } else if !matches!(definition, cst::Definition::OperationDefinition(_)) {
            let range = definition.syntax().text_range();
            let start: usize = range.start().into();
            let end: usize = range.end().into();
            if let Some(text) = contents.get(start..end) {
                extensions.push(ExtensionSnippet {
                    text: text.to_string(),
                    file: file.to_path_buf(),
                });
            }
        }
    }

    let mut operations = Vec::new();
    for definition in doc.definitions() {
        if let cst::Definition::OperationDefinition(def) = definition {
            if let Some(op) = build_operation_input(file, contents, def, &fragment_texts) {
                operations.push(op);
            }
        }
    }

    Ok(ParsedFile {
        operations,
        extensions,
    })
}

fn build_operation_input(
    file: &Utf8Path,
    contents: &str,
    def: cst::OperationDefinition,
    fragments: &[String],
) -> Option<OperationInput> {
    let name = def
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_else(|| String::from("<anonymous>"));
    if name == "<anonymous>" {
        return None;
    }
    let range = def.syntax().text_range();
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    let operation_text = contents.get(start..end)?.to_string();

    let fragments_text = fragments.join("\n\n");

    let body = if fragments_text.is_empty() {
        operation_text
    } else {
        format!("{operation_text}\n\n{fragments_text}")
    };

    let line = contents[..start].lines().count() + 1;
    let column = contents[..start]
        .rsplit_once('\n')
        .map(|(_, rest)| rest.len() + 1)
        .unwrap_or(1);

    Some(OperationInput {
        name,
        body,
        file: file.to_path_buf(),
        line,
        column,
    })
}

async fn collect_extension_failures(
    client_config: &StudioClientConfig,
    profile: &ProfileOpt,
    graph_ref: &rover_client::shared::GraphRef,
    extensions: &[ExtensionSnippet],
) -> Vec<ClientCheckFailure> {
    if extensions.is_empty() {
        return Vec::new();
    }

    let client = match client_config.get_authenticated_client(profile) {
        Ok(client) => client,
        Err(err) => {
            return vec![ClientCheckFailure {
                file: Utf8PathBuf::from("schema.graphql"),
                message: err.to_string(),
            }];
        }
    };

    let fetch_input = rover_client::operations::graph::fetch::GraphFetchInput {
        graph_ref: graph_ref.clone(),
    };
    let fetch_response =
        match rover_client::operations::graph::fetch::run(fetch_input, &client).await {
            Ok(res) => res,
            Err(err) => {
                return vec![ClientCheckFailure {
                    file: Utf8PathBuf::from("schema.graphql"),
                    message: err.to_string(),
                }];
            }
        };

    let failures = validate_extensions(&fetch_response.sdl.contents, extensions);
    failures
        .into_iter()
        .map(|ext_failure| ClientCheckFailure {
            file: ext_failure.file,
            message: format_extension_failure(ext_failure),
        })
        .collect()
}

fn format_extension_failure(failure: ExtensionFailure) -> String {
    if let (Some(line), Some(column)) = (failure.line, failure.column) {
        format!("{} at {}:{}", failure.message, line, column)
    } else {
        failure.message
    }
}

async fn validate_against_remote(
    client: &rover_client::blocking::StudioClient,
    graph_ref: &rover_client::shared::GraphRef,
    operations: &[OperationInput],
    git_context: &rover_client::shared::GitContext,
) -> RoverResult<Vec<ClientValidationResult>> {
    let op_inputs = operations
        .iter()
        .map(|op| OperationDocument {
            name: op.name.clone(),
            body: op.body.clone(),
        })
        .collect::<Vec<_>>();

    let validation_input = ValidateOperationsInput {
        graph_ref: graph_ref.clone(),
        operations: op_inputs,
        git_context: git_context.clone(),
    };

    let results = validate_operations::run(validation_input, client).await?;
    Ok(results
        .into_iter()
        .map(|val| ClientValidationResult {
            operation_name: val.operation_name,
            r#type: val.r#type,
            code: val.code,
            description: val.description,
            file: None,
            line: None,
            column: None,
        })
        .collect())
}
