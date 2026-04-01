mod parsed_file;

use std::collections::HashMap;

use itertools::Itertools;

use camino::Utf8PathBuf;
use clap::Parser as ClapParser;
use rover_client::operations::graph::{
    validate_operations,
    validate_operations::{OperationDocument, ValidateOperationsInput},
};
use rover_std::FileSearch;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::client::extensions::{ExtensionFailure, ExtensionSnippet, validate_extensions},
    options::{OptionalGraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};
use parsed_file::ParsedFile;

#[derive(Debug, thiserror::Error)]
enum ClientCheckError {
    #[error("Failed to parse {} .graphql file(s):\n{}", .parse_failures.len(), .parse_failures.iter().join("\n"))]
    ParseFailures {
        parse_failures: Vec<ClientCheckFailure>,
    },
    #[error("No .graphql operations found under the provided includes")]
    NoOperations,
    #[error("A graph ref is required for client check.")]
    MissingGraphRef,
    #[error("current directory is not utf-8")]
    NonUtf8CurrentDir,
}

#[derive(Debug, Serialize, ClapParser)]
pub struct Check {
    /// Graph reference to validate against (optional; defaults to env/config)
    #[clap(flatten)]
    graph: OptionalGraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    /// Glob patterns to include (e.g. `src/**/*.graphql`).
    #[arg(long = "include", value_name = "PATTERN", action = clap::ArgAction::Append)]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g. `**/__generated__/**`).
    #[arg(long = "exclude", value_name = "PATTERN", action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Root directory to scan. Defaults to the current working directory.
    #[arg(long = "root-dir", value_name = "DIR")]
    root_dir: Option<Utf8PathBuf>,
}

#[derive(thiserror::Error, Debug, Serialize, Clone, PartialEq, Eq)]
#[error("{file}: {message}")]
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
    pub has_errors: bool,
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

impl Check {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: rover_client::shared::GitContext,
        _format: crate::cli::RoverOutputFormatKind,
    ) -> RoverResult<RoverOutput> {
        let root = match &self.root_dir {
            Some(r) => r.clone(),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd)
                    .map_err(|_| ClientCheckError::NonUtf8CurrentDir)?
            }
        };

        let search = FileSearch::builder()
            .root(root)
            .includes(self.include.clone())
            .excludes(self.exclude.clone())
            .build();

        let files = search.find(&["graphql"])?;

        let results: Vec<Result<ParsedFile, ClientCheckFailure>> = files
            .into_iter()
            .map(|file| {
                rover_std::Fs::read_file(&file)
                    .map_err(|e| ClientCheckFailure {
                        file: file.clone(),
                        message: e.to_string(),
                    })
                    .and_then(|contents| {
                        ParsedFile::new(&file, &contents).map_err(|e| ClientCheckFailure {
                            file: file.clone(),
                            message: e.to_string(),
                        })
                    })
            })
            .collect();

        let (parsed_files, parse_failures): (Vec<ParsedFile>, Vec<ClientCheckFailure>) =
            results.into_iter().partition_result();

        if !parse_failures.is_empty() {
            return Err(ClientCheckError::ParseFailures { parse_failures })?;
        }

        let operations: Vec<_> = parsed_files.iter().flat_map(|f| f.operations.iter().cloned()).collect();
        let extensions: Vec<_> = parsed_files.iter().flat_map(|f| f.extensions.iter().cloned()).collect();
        let mut failures = Vec::new();

        if operations.is_empty() {
            Err(ClientCheckError::NoOperations)?;
        }

        let graph_ref = self
            .graph
            .graph_ref
            .clone()
            .ok_or(ClientCheckError::MissingGraphRef)?;

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

        let has_errors = mapped_results
            .iter()
            .any(|r| matches!(r.r#type.as_str(), "FAILURE" | "INVALID"))
            || !failures.is_empty();

        let summary = ClientCheckSummary {
            graph_ref: Some(graph_ref.to_string()),
            files_scanned: operations.len(),
            operations_sent: operations.len(),
            failures,
            validation_results: mapped_results,
            has_errors,
        };

        Ok(RoverOutput::ClientCheckResponse { summary })
    }
}

async fn collect_extension_failures(
    client_config: &StudioClientConfig,
    profile: &ProfileOpt,
    graph_ref: &rover_studio::types::GraphRef,
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
            file: ext_failure.file.clone(),
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
    graph_ref: &rover_studio::types::GraphRef,
    operations: &[parsed_file::OperationInput],
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
