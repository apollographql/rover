mod output;
mod parsed_file;

use std::collections::{BTreeMap, HashMap};

use itertools::Itertools;

use camino::Utf8PathBuf;
use clap::Parser as ClapParser;
use rover_client::operations::graph::{
    fetch::{GraphFetch, GraphFetchInput, GraphFetchRequest},
    validate_operations::{
        OperationDocument, ValidateOperations, ValidateOperationsInput, ValidateOperationsRequest,
        ValidationErrorCode, ValidationResultType,
    },
};
use rover_graphql::GraphQLLayer;
use rover_http::HttpService;
use rover_std::FileSearch;
use rover_studio::types::GraphRef;
use serde::Serialize;
use tower::{Service, ServiceBuilder, ServiceExt};

use crate::{
    RoverOutput, RoverResult,
    command::client::extensions::{ExtensionFailure, ExtensionSnippet, validate_extensions},
    options::{OptionalGraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
};
use parsed_file::{OperationInput, ParsedFile};

type GraphQlService = rover_graphql::GraphQLService<HttpService>;

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

#[derive(thiserror::Error, Debug, Serialize, Clone, PartialEq, Eq)]
#[error("{file}: {message}")]
/// A file-level parse or schema-extension failure encountered before network validation.
pub struct ClientCheckFailure {
    pub file: Utf8PathBuf,
    pub message: String,
}

/// Aggregated result from a `rover client check` run.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ClientCheckSummary {
    pub graph_ref: Option<String>,
    pub files_scanned: usize,
    pub operations_sent: usize,
    pub failures: Vec<ClientCheckFailure>,
    pub validation_results: Vec<ClientValidationResult>,
    pub has_errors: bool,
}

/// A single operation-level validation result, enriched with source-location information.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ClientValidationResult {
    pub operation_name: String,
    pub r#type: ValidationResultType,
    pub code: ValidationErrorCode,
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
    ) -> RoverResult<RoverOutput> {
        let parsed_files = self.find_and_parse_files()?;
        let (operations, extensions) = gather_inputs(&parsed_files);

        if operations.is_empty() {
            Err(ClientCheckError::NoOperations)?;
        }

        let graph_ref = self.require_graph_ref()?;
        let service = self.build_graphql_service(&client_config)?;

        let extension_failures =
            fetch_and_validate_extensions(&extensions, &graph_ref, service.clone()).await?;
        let raw_results =
            validate_operations_remotely(&operations, &graph_ref, &git_context, service).await?;
        let validation_results = annotate_with_locations(raw_results, &operations);

        let has_errors = validation_results.iter().any(|r| {
            matches!(
                r.r#type,
                ValidationResultType::Failure | ValidationResultType::Invalid
            )
        }) || !extension_failures.is_empty();

        Ok(RoverOutput::CliOutput(Box::new(
            output::ClientCheckOutput::from(ClientCheckSummary {
                graph_ref: Some(graph_ref.to_string()),
                files_scanned: parsed_files.len(),
                operations_sent: operations.len(),
                failures: extension_failures,
                validation_results,
                has_errors,
            }),
        )))
    }

    fn find_and_parse_files(&self) -> RoverResult<Vec<ParsedFile>> {
        let root = match &self.root_dir {
            Some(r) => r.clone(),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd).map_err(|_| ClientCheckError::NonUtf8CurrentDir)?
            }
        };

        let canonical_root = root
            .canonicalize()
            .unwrap_or_else(|_| root.as_std_path().to_path_buf());
        let includes: Vec<String> = self
            .include
            .iter()
            .map(|p| {
                let path = std::path::Path::new(p);
                if path.is_absolute() {
                    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                    canonical
                        .strip_prefix(&canonical_root)
                        .map(|rel| rel.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| p.clone())
                } else {
                    p.clone()
                }
            })
            .collect();

        let files = FileSearch::builder()
            .root(root)
            .includes(includes)
            .excludes(self.exclude.clone())
            .build()
            .find(&["graphql"])?;

        parse_graphql_files(files)
    }

    fn require_graph_ref(&self) -> Result<GraphRef, ClientCheckError> {
        self.graph
            .graph_ref
            .clone()
            .ok_or(ClientCheckError::MissingGraphRef)
    }

    fn build_graphql_service(
        &self,
        client_config: &StudioClientConfig,
    ) -> RoverResult<GraphQlService> {
        let http_service = client_config.authenticated_service(&self.profile)?;
        Ok(ServiceBuilder::new()
            .layer(GraphQLLayer::default())
            .service(http_service))
    }
}

fn parse_graphql_files(files: Vec<Utf8PathBuf>) -> RoverResult<Vec<ParsedFile>> {
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
        Err(ClientCheckError::ParseFailures { parse_failures })?;
    }

    Ok(parsed_files)
}

fn gather_inputs(parsed_files: &[ParsedFile]) -> (Vec<OperationInput>, Vec<ExtensionSnippet>) {
    let fragments_text = {
        let all_fragments: BTreeMap<_, _> = parsed_files
            .iter()
            .flat_map(|f| f.fragments.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        all_fragments.into_values().join("\n\n")
    };

    let operations = parsed_files
        .iter()
        .flat_map(|f| f.operations.iter())
        .map(|op| {
            let body = if fragments_text.is_empty() {
                op.body.clone()
            } else {
                format!("{}\n\n{}", op.body, fragments_text)
            };
            OperationInput { body, ..op.clone() }
        })
        .collect();

    let extensions = parsed_files
        .iter()
        .flat_map(|f| f.extensions.iter().cloned())
        .collect();

    (operations, extensions)
}

async fn fetch_and_validate_extensions(
    extensions: &[ExtensionSnippet],
    graph_ref: &GraphRef,
    service: GraphQlService,
) -> RoverResult<Vec<ClientCheckFailure>> {
    if extensions.is_empty() {
        return Ok(Vec::new());
    }

    let mut fetch_service = GraphFetch::new(service);
    let fetch_service = fetch_service.ready().await?;
    let sdl = fetch_service
        .call(GraphFetchRequest::new(GraphFetchInput {
            graph_ref: graph_ref.clone(),
        }))
        .await?
        .sdl
        .contents;

    let failures = validate_extensions(&sdl, &graph_ref.to_string(), extensions)
        .into_iter()
        .map(|f| ClientCheckFailure {
            file: f.file.clone(),
            message: format_extension_failure(f),
        })
        .collect();

    Ok(failures)
}

async fn validate_operations_remotely(
    operations: &[OperationInput],
    graph_ref: &GraphRef,
    git_context: &rover_client::shared::GitContext,
    service: GraphQlService,
) -> RoverResult<Vec<ClientValidationResult>> {
    let op_inputs = operations
        .iter()
        .map(|op| OperationDocument {
            name: op.name.clone(),
            body: op.body.clone(),
        })
        .collect();

    let mut validate_service = ValidateOperations::new(service);
    let validate_service = validate_service.ready().await?;
    let raw_results = validate_service
        .call(ValidateOperationsRequest::new(ValidateOperationsInput {
            graph_ref: graph_ref.clone(),
            operations: op_inputs,
            git_context: git_context.clone(),
        }))
        .await?;

    Ok(raw_results
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

fn annotate_with_locations(
    results: Vec<ClientValidationResult>,
    operations: &[OperationInput],
) -> Vec<ClientValidationResult> {
    let op_lookup: HashMap<_, _> = operations.iter().map(|op| (op.name.clone(), op)).collect();
    results
        .into_iter()
        .map(|mut res| {
            if let Some(op) = op_lookup.get(&res.operation_name) {
                res.file = Some(op.file.clone());
                res.line = Some(op.line);
                res.column = Some(op.column);
            }
            res
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
