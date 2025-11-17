use std::collections::HashMap;
use camino::{Utf8Path, Utf8PathBuf};
use anyhow::anyhow;
use apollo_compiler::{ExecutableDocument, Schema, ValidationMode};
use apollo_parser::{Parser, cst};
use apollo_parser::cst::CstNode;
use clap::Parser as ClapParser;
use serde::Serialize;
use serde_json::json;

use crate::{
    RoverError, RoverOutput, RoverResult,
    client::{
        discovery::{discover_files, DiscoveryOptions},
        extensions::{ExtensionFailure, ExtensionSnippet, validate_extensions},
    },
    options::{OptionalGraphRefOpt, ProfileOpt},
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

#[derive(Debug, Clone)]
struct ExtensionSnippet {
    text: String,
    file: Utf8PathBuf,
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

        let graph_ref = self
            .graph
            .graph_ref
            .clone()
            .ok_or_else(|| RoverError::new(anyhow!("A graph ref is required for client check.")))?;

        let extension_failures =
            collect_extension_failures(&client_config, &self.profile, &graph_ref, &extensions)
                .await;
        failures.extend(extension_failures);

        let client = client_config.get_authenticated_client(&self.profile)?;

        let validation_results =
            validate_against_remote(&client, &graph_ref, &operations, &git_context).await?;

        let op_lookup: HashMap<_, _> = operations
            .iter()
            .map(|op| (op.name.clone(), op))
            .collect();

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
        let has_errors = summary.validation_results.iter().any(|r| {
            matches!(r.r#type.as_str(), "FAILURE" | "INVALID")
        }) || !summary.failures.is_empty();

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
            }]
        }
    };

    let fetch_input =
        rover_client::operations::graph::fetch::GraphFetchInput { graph_ref: graph_ref.clone() };
    let fetch_response = match rover_client::operations::graph::fetch::run(fetch_input, &client).await {
        Ok(res) => res,
        Err(err) => {
            return vec![ClientCheckFailure {
                file: Utf8PathBuf::from("schema.graphql"),
                message: err.to_string(),
            }]
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
    const VALIDATE_MUTATION: &str = r#"
        mutation ClientValidate($graph_id: ID!, $tag: String!, $operations: [OperationDocumentInput!]!, $gitContext: GitContextInput) {
          service(id: $graph_id) {
            validateOperations(tag: $tag, operations: $operations, gitContext: $gitContext) {
              validationResults {
                type
                code
                description
                operation { name }
              }
            }
          }
        }
    "#;

    let op_inputs = operations
        .iter()
        .map(|op| json!({ "name": op.name, "body": op.body }))
        .collect::<Vec<_>>();

    let git_ctx = json!({
        "branch": git_context.branch,
        "commit": git_context.commit,
        "committer": git_context.author,
        "remoteUrl": git_context.remote_url,
    });

    let variables = json!({
        "graph_id": graph_ref.name,
        "tag": graph_ref.variant,
        "operations": op_inputs,
        "gitContext": git_ctx,
    });

    let response = client.post_raw(VALIDATE_MUTATION, variables).await?;
    let results = response
        .get("data")
        .and_then(|d| d.get("service"))
        .and_then(|s| s.get("validateOperations"))
        .and_then(|v| v.get("validationResults"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| RoverError::new(anyhow!("Malformed response from validation API")))?;

    Ok(results
        .iter()
        .filter_map(|val| {
            let operation_name = val
                .get("operation")
                .and_then(|o| o.get("name"))
                .and_then(|n| n.as_str())?
                .to_string();
            Some(ClientValidationResult {
                operation_name,
                r#type: val
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string(),
                code: val
                    .get("code")
                    .and_then(|c| c.as_str())
                    .map(|c| c.to_string()),
                description: val
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string(),
                file: None,
                line: None,
                column: None,
            })
        })
        .collect())
}
