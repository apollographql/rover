mod output;
mod printer;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use apollo_compiler::{Node, ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use itertools::Itertools;
use rover_std::{FileSearch, Fs, Style};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{RoverOutput, RoverResult};

use output::GenerateOutput;
use printer::{PrintableDefinition, operation_type_str, print_document};

const DEFAULT_INCLUDE: &str = "graphql/**/*.graphql";
const DEFAULT_MANIFEST: &str = "persisted-query-manifest.json";
const MANIFEST_FORMAT: &str = "apollo-persisted-query-manifest";
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Serialize, Parser)]
pub struct Generate {
    /// Glob patterns to include (e.g. `graphql/**/*.graphql`).
    #[arg(long = "include", value_name = "PATTERN", action = clap::ArgAction::Append)]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g. `**/__generated__/**`).
    #[arg(long = "exclude", value_name = "PATTERN", action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Root directory to scan. Defaults to the current working directory.
    #[arg(long = "root-dir", value_name = "DIR")]
    root_dir: Option<Utf8PathBuf>,

    /// Path for the generated manifest file.
    /// Defaults to `persisted-query-manifest.json` in the current directory.
    #[arg(long = "manifest-path", short = 'm', value_name = "FILE")]
    manifest_path: Option<Utf8PathBuf>,
}

#[derive(Debug, thiserror::Error)]
enum GenerateError {
    #[error("current directory is not utf-8")]
    NonUtf8CurrentDir,
    #[error("Failed to parse {} .graphql file(s):\n{}", .parse_failures.len(), .parse_failures.iter().join("\n"))]
    ParseFailures {
        parse_failures: Vec<GenerateFailure>,
    },
    #[error(
        "Anonymous GraphQL operations are not supported. Please name your {operation_type} in {file}."
    )]
    AnonymousOperation {
        file: Utf8PathBuf,
        operation_type: String,
    },
    #[error(
        "Operation named \"{name}\" is already defined in {first_file}. Duplicate found in {second_file}."
    )]
    DuplicateOperation {
        name: String,
        first_file: Utf8PathBuf,
        second_file: Utf8PathBuf,
    },
    #[error(
        "Fragment named \"{name}\" is already defined in {first_file}. Duplicate found in {second_file}."
    )]
    DuplicateFragment {
        name: String,
        first_file: Utf8PathBuf,
        second_file: Utf8PathBuf,
    },
    #[error(
        "Operation named \"{operation_name}\" references missing fragment \"{fragment_name}\"."
    )]
    MissingFragment {
        operation_name: String,
        fragment_name: String,
    },
    #[error(
        "Generated operation ID {id} for operation \"{operation_name}\" was already used for operation \"{existing_operation_name}\"."
    )]
    DuplicateOperationId {
        id: String,
        operation_name: String,
        existing_operation_name: String,
    },
}

#[derive(Debug, thiserror::Error)]
#[error("{file}: {message}")]
struct GenerateFailure {
    file: Utf8PathBuf,
    message: String,
}

#[derive(Debug, Clone)]
struct ParsedOperation {
    file: Utf8PathBuf,
    source: Arc<str>,
    operation: Node<ast::OperationDefinition>,
    direct_fragment_spreads: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct ParsedFragment {
    file: Utf8PathBuf,
    source: Arc<str>,
    fragment: Node<ast::FragmentDefinition>,
    direct_fragment_spreads: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct ParsedInputs {
    operations: BTreeMap<String, ParsedOperation>,
    fragments: BTreeMap<String, ParsedFragment>,
}

#[derive(Debug, Serialize)]
struct GeneratedManifest {
    format: &'static str,
    version: u8,
    operations: Vec<GeneratedOperation>,
}

#[derive(Debug, Serialize)]
struct GeneratedOperation {
    id: String,
    name: String,
    #[serde(rename = "type")]
    operation_type: &'static str,
    body: String,
}

impl Generate {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let manifest = self.generate_manifest()?;
        let output_path = self.resolve_manifest_path()?;
        let operation_count = manifest.operations.len();

        if operation_count == 0 {
            eprintln!(
                "{} no operations found during manifest generation. You may need to adjust the glob pattern used to search files in this project.",
                Style::WarningPrefix.paint("warning:"),
            );
        }

        let manifest_json = format!("{}\n", serde_json::to_string_pretty(&manifest)?);
        Fs::write_file(&output_path, manifest_json)?;

        Ok(RoverOutput::CliOutput(Box::new(GenerateOutput {
            path: output_path,
            operation_count,
        })))
    }

    fn resolve_manifest_path(&self) -> RoverResult<Utf8PathBuf> {
        match &self.manifest_path {
            Some(path) => Ok(path.clone()),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd)
                    .map(|cwd| cwd.join(DEFAULT_MANIFEST))
                    .map_err(|_| GenerateError::NonUtf8CurrentDir.into())
            }
        }
    }

    fn generate_manifest(&self) -> RoverResult<GeneratedManifest> {
        let files = self.find_graphql_files()?;
        let parsed_inputs = parse_files(files)?;
        let operations = generate_operations(&parsed_inputs)?;

        Ok(GeneratedManifest {
            format: MANIFEST_FORMAT,
            version: MANIFEST_VERSION,
            operations,
        })
    }

    fn find_graphql_files(&self) -> RoverResult<Vec<Utf8PathBuf>> {
        let root = match &self.root_dir {
            Some(r) => r.clone(),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd).map_err(|_| GenerateError::NonUtf8CurrentDir)?
            }
        };

        let canonical_root = dunce::canonicalize(root.as_std_path())
            .unwrap_or_else(|_| root.as_std_path().to_path_buf());
        let canonical_root_utf8 =
            Utf8PathBuf::from_path_buf(canonical_root.clone()).unwrap_or(root);

        let includes = if self.include.is_empty() {
            vec![DEFAULT_INCLUDE.to_string()]
        } else {
            normalize_includes(&self.include, &canonical_root)
        };

        FileSearch::builder()
            .root(canonical_root_utf8)
            .includes(includes)
            .excludes(self.exclude.clone())
            .build()
            .find(&["graphql"])
            .map_err(crate::RoverError::from)
    }
}

fn normalize_includes(includes: &[String], canonical_root: &std::path::Path) -> Vec<String> {
    includes
        .iter()
        .map(|p| {
            let path = std::path::Path::new(p);
            if path.is_absolute() {
                dunce::canonicalize(path)
                    .unwrap_or_else(|_| path.to_path_buf())
                    .strip_prefix(canonical_root)
                    .map(|rel| rel.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| p.clone())
            } else {
                p.clone()
            }
        })
        .collect()
}

fn parse_files(files: Vec<Utf8PathBuf>) -> RoverResult<ParsedInputs> {
    let (parsed, failures): (Vec<_>, Vec<_>) = files
        .into_iter()
        .map(|file| parse_file(&file))
        .partition_result();

    if !failures.is_empty() {
        Err(GenerateError::ParseFailures {
            parse_failures: failures,
        })?;
    }

    parsed
        .into_iter()
        .try_fold(ParsedInputs::default(), |mut acc, file| {
            merge_parsed_file(&mut acc, file)?;
            Ok(acc)
        })
}

fn parse_file(file: &Utf8Path) -> Result<ParsedInputs, GenerateFailure> {
    let contents = Fs::read_file(file).map_err(|err| GenerateFailure {
        file: file.to_path_buf(),
        message: err.to_string(),
    })?;
    let source: Arc<str> = Arc::from(contents.as_str());
    let document = ApolloParser::new()
        .parse_ast(contents, file.as_std_path())
        .map_err(|err| GenerateFailure {
            file: file.to_path_buf(),
            message: err.to_string(),
        })?;

    let mut parsed = ParsedInputs::default();
    for definition in document.definitions {
        match definition {
            ast::Definition::OperationDefinition(operation) => {
                let name = operation
                    .name
                    .as_ref()
                    .map(ToString::to_string)
                    .ok_or_else(|| GenerateFailure {
                        file: file.to_path_buf(),
                        message: GenerateError::AnonymousOperation {
                            file: file.to_path_buf(),
                            operation_type: operation.operation_type.to_string(),
                        }
                        .to_string(),
                    })?;
                if parsed.operations.contains_key(&name) {
                    return Err(GenerateFailure {
                        file: file.to_path_buf(),
                        message: GenerateError::DuplicateOperation {
                            name,
                            first_file: file.to_path_buf(),
                            second_file: file.to_path_buf(),
                        }
                        .to_string(),
                    });
                }
                parsed.operations.insert(
                    name,
                    ParsedOperation {
                        file: file.to_path_buf(),
                        source: Arc::clone(&source),
                        direct_fragment_spreads: collect_spreads(&operation.selection_set),
                        operation,
                    },
                );
            }
            ast::Definition::FragmentDefinition(fragment) => {
                if parsed.fragments.contains_key(fragment.name.as_str()) {
                    return Err(GenerateFailure {
                        file: file.to_path_buf(),
                        message: GenerateError::DuplicateFragment {
                            name: fragment.name.to_string(),
                            first_file: file.to_path_buf(),
                            second_file: file.to_path_buf(),
                        }
                        .to_string(),
                    });
                }
                parsed.fragments.insert(
                    fragment.name.to_string(),
                    ParsedFragment {
                        file: file.to_path_buf(),
                        source: Arc::clone(&source),
                        direct_fragment_spreads: collect_spreads(&fragment.selection_set),
                        fragment,
                    },
                );
            }
            _ => {}
        }
    }
    Ok(parsed)
}

fn merge_parsed_file(inputs: &mut ParsedInputs, parsed_file: ParsedInputs) -> RoverResult<()> {
    for (name, operation) in parsed_file.operations {
        if let Some(existing) = inputs.operations.get(&name) {
            Err(GenerateError::DuplicateOperation {
                name: name.clone(),
                first_file: existing.file.clone(),
                second_file: operation.file.clone(),
            })?;
        }
        inputs.operations.insert(name, operation);
    }

    for (name, fragment) in parsed_file.fragments {
        if let Some(existing) = inputs.fragments.get(&name) {
            Err(GenerateError::DuplicateFragment {
                name: name.clone(),
                first_file: existing.file.clone(),
                second_file: fragment.file.clone(),
            })?;
        }
        inputs.fragments.insert(name, fragment);
    }

    Ok(())
}

fn generate_operations(inputs: &ParsedInputs) -> RoverResult<Vec<GeneratedOperation>> {
    let mut operation_ids = HashMap::new();
    inputs
        .operations
        .iter()
        .map(|(name, operation)| {
            let body = generate_operation_body(name, operation, &inputs.fragments)?;
            let id = sha256_hex(&body);
            if let Some(existing_operation_name) = operation_ids.insert(id.clone(), name.clone()) {
                Err(GenerateError::DuplicateOperationId {
                    id: id.clone(),
                    operation_name: name.clone(),
                    existing_operation_name,
                })?;
            }
            Ok(GeneratedOperation {
                id,
                name: name.clone(),
                operation_type: operation_type_str(operation.operation.operation_type),
                body,
            })
        })
        .collect()
}

fn generate_operation_body(
    operation_name: &str,
    operation: &ParsedOperation,
    fragments: &BTreeMap<String, ParsedFragment>,
) -> RoverResult<String> {
    let reachable_fragments =
        reachable_fragments(operation_name, &operation.direct_fragment_spreads, fragments)?;
    let mut operation_node = operation.operation.clone();
    add_typename_to_operation(operation_node.make_mut());

    let fragment_definitions: Vec<(Node<ast::FragmentDefinition>, Arc<str>)> = reachable_fragments
        .iter()
        .map(|fragment_name| {
            let fragment = fragments
                .get(fragment_name)
                .expect("reachable fragments are validated before returning");
            let mut fragment_node = fragment.fragment.clone();
            let fragment_definition = fragment_node.make_mut();
            fragment_definition
                .directives
                .0
                .retain(|directive| directive.name != "client");
            add_typename_to_selection_set(&mut fragment_definition.selection_set);
            (fragment_node, Arc::clone(&fragment.source))
        })
        .collect();

    prune_unused_variables(operation_node.make_mut(), &fragment_definitions);

    let definitions = std::iter::once(PrintableDefinition::Operation {
        operation: operation_node,
        source: Arc::clone(&operation.source),
    })
    .chain(
        fragment_definitions
            .into_iter()
            .map(|(fragment, source)| PrintableDefinition::Fragment { fragment, source }),
    )
    .collect::<Vec<_>>();

    Ok(print_document(&definitions))
}

fn reachable_fragments(
    operation_name: &str,
    seeds: &BTreeSet<String>,
    fragments: &BTreeMap<String, ParsedFragment>,
) -> RoverResult<BTreeSet<String>> {
    let mut reachable = BTreeSet::new();
    let mut queue: Vec<&str> = seeds.iter().map(String::as_str).collect();

    while let Some(name) = queue.pop() {
        if !reachable.insert(name.to_string()) {
            continue;
        }

        let fragment = fragments
            .get(name)
            .ok_or_else(|| GenerateError::MissingFragment {
                operation_name: operation_name.to_string(),
                fragment_name: name.to_string(),
            })?;
        queue.extend(fragment.direct_fragment_spreads.iter().map(String::as_str));
    }

    Ok(reachable)
}

fn collect_spreads(selections: &[ast::Selection]) -> BTreeSet<String> {
    selections
        .iter()
        .filter(|selection| !selection_has_directive(selection, "client"))
        .flat_map(|selection| -> Box<dyn Iterator<Item = String>> {
            match selection {
                ast::Selection::FragmentSpread(fragment_spread) => {
                    Box::new(std::iter::once(fragment_spread.fragment_name.to_string()))
                }
                ast::Selection::Field(field) => {
                    Box::new(collect_spreads(&field.selection_set).into_iter())
                }
                ast::Selection::InlineFragment(inline_fragment) => {
                    Box::new(collect_spreads(&inline_fragment.selection_set).into_iter())
                }
            }
        })
        .collect()
}

fn remove_client_selections_from_selection_set(selections: &mut Vec<ast::Selection>) {
    selections.retain(|selection| !selection_has_directive(selection, "client"));
}

fn selection_has_directive(selection: &ast::Selection, directive_name: &str) -> bool {
    match selection {
        ast::Selection::Field(field) => field.directives.has(directive_name),
        ast::Selection::FragmentSpread(fragment_spread) => {
            fragment_spread.directives.has(directive_name)
        }
        ast::Selection::InlineFragment(inline_fragment) => {
            inline_fragment.directives.has(directive_name)
        }
    }
}

fn add_typename_to_operation(operation: &mut ast::OperationDefinition) {
    remove_client_selections_from_selection_set(&mut operation.selection_set);
    operation
        .directives
        .0
        .retain(|directive| directive.name != "client");
    for selection in &mut operation.selection_set {
        add_typename_to_child_selection(selection);
    }
}

fn add_typename_to_selection_set(selections: &mut Vec<ast::Selection>) {
    add_typename_to_selection_set_inner(selections, true);
}

fn add_typename_to_selection_set_inner(
    selections: &mut Vec<ast::Selection>,
    append_typename: bool,
) {
    remove_client_selections_from_selection_set(selections);
    for selection in selections.iter_mut() {
        add_typename_to_child_selection(selection);
    }
    if append_typename && !selections.iter().any(is_typename_field) {
        selections.push(ast::Selection::Field(Node::new(ast::Field {
            alias: None,
            name: apollo_compiler::name!("__typename"),
            arguments: Vec::new(),
            directives: ast::DirectiveList::new(),
            selection_set: Vec::new(),
        })));
    }
}

fn add_typename_to_child_selection(selection: &mut ast::Selection) {
    match selection {
        ast::Selection::Field(field) => {
            let field = field.make_mut();
            if !field.selection_set.is_empty() {
                let should_add_typename = !field.directives.has("export");
                add_typename_to_selection_set_inner(&mut field.selection_set, should_add_typename);
            }
        }
        ast::Selection::InlineFragment(inline_fragment) => {
            add_typename_to_selection_set(&mut inline_fragment.make_mut().selection_set);
        }
        ast::Selection::FragmentSpread(_) => {}
    }
}

fn is_typename_field(selection: &ast::Selection) -> bool {
    matches!(selection, ast::Selection::Field(field) if field.name == "__typename")
}

fn prune_unused_variables(
    operation: &mut ast::OperationDefinition,
    fragments: &[(Node<ast::FragmentDefinition>, Arc<str>)],
) {
    let used_variables: BTreeSet<String> = std::iter::once(collect_variables_from_operation(
        operation,
    ))
    .chain(
        fragments
            .iter()
            .map(|(fragment, _)| collect_variables_from_fragment(fragment)),
    )
    .fold(BTreeSet::new(), |mut acc, vars| {
        acc.extend(vars);
        acc
    });

    operation
        .variables
        .retain(|variable| used_variables.contains(variable.name.as_str()));
}

fn collect_variables_from_operation(operation: &ast::OperationDefinition) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    collect_variables_from_directives(&operation.directives, &mut variables);
    collect_variables_from_selection_set(&operation.selection_set, &mut variables);
    variables
}

fn collect_variables_from_fragment(fragment: &ast::FragmentDefinition) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    collect_variables_from_directives(&fragment.directives, &mut variables);
    collect_variables_from_selection_set(&fragment.selection_set, &mut variables);
    variables
}

fn collect_variables_from_selection_set(
    selections: &[ast::Selection],
    variables: &mut BTreeSet<String>,
) {
    for selection in selections {
        match selection {
            ast::Selection::Field(field) => {
                collect_variables_from_arguments(&field.arguments, variables);
                collect_variables_from_directives(&field.directives, variables);
                collect_variables_from_selection_set(&field.selection_set, variables);
            }
            ast::Selection::FragmentSpread(fragment_spread) => {
                collect_variables_from_directives(&fragment_spread.directives, variables);
            }
            ast::Selection::InlineFragment(inline_fragment) => {
                collect_variables_from_directives(&inline_fragment.directives, variables);
                collect_variables_from_selection_set(&inline_fragment.selection_set, variables);
            }
        }
    }
}

fn collect_variables_from_directives(
    directives: &ast::DirectiveList,
    variables: &mut BTreeSet<String>,
) {
    for directive in directives.iter() {
        collect_variables_from_arguments(&directive.arguments, variables);
    }
}

fn collect_variables_from_arguments(
    arguments: &[Node<ast::Argument>],
    variables: &mut BTreeSet<String>,
) {
    for argument in arguments {
        collect_variables_from_value(&argument.value, variables);
    }
}

fn collect_variables_from_value(value: &Node<ast::Value>, variables: &mut BTreeSet<String>) {
    match value.as_ref() {
        ast::Value::Variable(name) => {
            variables.insert(name.to_string());
        }
        ast::Value::List(values) => {
            for value in values {
                collect_variables_from_value(value, variables);
            }
        }
        ast::Value::Object(fields) => {
            for (_, value) in fields {
                collect_variables_from_value(value, variables);
            }
        }
        ast::Value::Null
        | ast::Value::Enum(_)
        | ast::Value::String(_)
        | ast::Value::Float(_)
        | ast::Value::Int(_)
        | ast::Value::Boolean(_) => {}
    }
}

fn sha256_hex(body: &str) -> String {
    Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn parsed_inputs(source: &str) -> ParsedInputs {
        parsed_inputs_from_files(&[("ops.graphql", source)])
    }

    fn parsed_inputs_from_files(files: &[(&str, &str)]) -> ParsedInputs {
        let temp = tempfile::tempdir().unwrap();
        let mut inputs = ParsedInputs::default();
        for (filename, source) in files {
            let file = Utf8PathBuf::from_path_buf(temp.path().join(filename)).unwrap();
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, source).unwrap();
            let parsed_file = parse_file(&file).unwrap();
            merge_parsed_file(&mut inputs, parsed_file).unwrap();
        }
        inputs
    }

    #[test]
    fn generated_body_matches_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            fragment ProductFields on Product {
              id
              name
              nested { value }
            }

            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
              }
            }

            mutation SaveProduct {
              saveProduct(input: { name: "x" }) { id }
            }
        "#});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(2);
        assert_that!(operations[0].name.as_str()).is_equal_to("GetProduct");
        assert_that!(operations[0].operation_type).is_equal_to("query");
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
                __typename
              }
            }

            fragment ProductFields on Product {
              id
              name
              nested {
                value
                __typename
              }
              __typename
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("deca7ebeb3e6d8e46f056fdc032ed462dc6a9763d9225eb04ab9e9943b6c248a");

        assert_that!(operations[1].name.as_str()).is_equal_to("SaveProduct");
        assert_that!(operations[1].operation_type).is_equal_to("mutation");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {r#"
            mutation SaveProduct {
              saveProduct(input: {name: "x"}) {
                id
                __typename
              }
            }"#});
        assert_that!(operations[1].id.as_str())
            .is_equal_to("e2cae5428130630ffe997257613154698cd85f7ef97c4ffe653ca80183b8e10f");
    }

    #[test]
    fn complex_documents_match_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs_from_files(&[
            (
                "complex.graphql",
                indoc::indoc! {r#"
                    query ComplexQuery(
                      $id: ID!
                      $limit: Int = 10
                      $tags: [String!] = ["featured", "sale"]
                      $filter: FilterInput = {status: ACTIVE, range: {min: 1.5, max: 3}}
                      $enabled: Boolean = true
                    ) @trace(enabled: true) {
                      viewer {
                        primary: user(id: $id, filter: $filter, tags: $tags) @include(if: $enabled) {
                          id
                          profile {
                            displayName
                          }
                          ... on Admin {
                            permissions
                          }
                          ...UserFields
                        }
                      }
                    }
                "#},
            ),
            (
                "fragments/user.graphql",
                indoc::indoc! {"
                    fragment UserFields on User @cache(ttl: 60) {
                      name
                      friends(first: $limit) {
                        nodes {
                          id
                        }
                      }
                      ...SharedFields
                    }
                "},
            ),
            (
                "fragments/shared.graphql",
                "fragment SharedFields on User { status }",
            ),
            (
                "subscription.graphql",
                indoc::indoc! {"
                    subscription UserCreatedSubscription($groupId: ID!) {
                      userCreated(groupId: $groupId) {
                        ...UserFields
                      }
                    }
                "},
            ),
        ]);

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(2);
        assert_that!(operations[0].name.as_str()).is_equal_to("ComplexQuery");
        assert_that!(operations[0].operation_type).is_equal_to("query");
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query ComplexQuery($id: ID!, $limit: Int = 10, $tags: [String!] = ["featured", "sale"], $filter: FilterInput = {status: ACTIVE, range: {min: 1.5, max: 3}}, $enabled: Boolean = true) @trace(enabled: true) {
              viewer {
                primary: user(id: $id, filter: $filter, tags: $tags) @include(if: $enabled) {
                  id
                  profile {
                    displayName
                    __typename
                  }
                  ... on Admin {
                    permissions
                    __typename
                  }
                  ...UserFields
                  __typename
                }
                __typename
              }
            }

            fragment SharedFields on User {
              status
              __typename
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                  __typename
                }
                __typename
              }
              ...SharedFields
              __typename
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("4501a1585e6aaf2adea38c6ffc4114135b71871e69bd43fff71de6a4ce8b57c2");

        assert_that!(operations[1].name.as_str()).is_equal_to("UserCreatedSubscription");
        assert_that!(operations[1].operation_type).is_equal_to("subscription");
        assert_that!(operations[1].body.as_str()).is_equal_to(indoc::indoc! {"
            subscription UserCreatedSubscription($groupId: ID!) {
              userCreated(groupId: $groupId) {
                ...UserFields
                __typename
              }
            }

            fragment SharedFields on User {
              status
              __typename
            }

            fragment UserFields on User @cache(ttl: 60) {
              name
              friends(first: $limit) {
                nodes {
                  id
                  __typename
                }
                __typename
              }
              ...SharedFields
              __typename
            }"});
        assert_that!(operations[1].id.as_str())
            .is_equal_to("e936af1be273b8d80d7c06927423827cbe464c3efd6b67ab02e948d20c3c9b59");
    }

    #[test]
    fn client_directive_selections_match_default_typescript_transform() {
        let inputs = parsed_inputs(indoc::indoc! {"
            fragment LocalFields on CurrentUser {
              temporary @client
            }

            query CurrentUserQuery {
              isLoggedIn @client
              currentUser {
                id
                ...LocalFields @client
              }
            }
        "});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                id
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("2bc729f3095726f8bc03301874e1e185d22aa06aad024b49c868a641c24c1902");
    }

    #[test]
    fn client_directive_removal_preserves_nested_typename() {
        let inputs = parsed_inputs(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                localOnly @client
              }
            }
        "});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("92e0c664584eac8c318fd0193771ceab698eb53b55f9cbe5e8f82a7935086c7e");
    }

    #[test]
    fn client_directive_removal_prunes_now_unused_variables() {
        let inputs = parsed_inputs(indoc::indoc! {"
            query CurrentUserQuery($localId: ID!, $userId: ID!) {
              isLoggedIn(id: $localId) @client
              currentUser(id: $userId) {
                id
              }
            }
        "});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!) {
              currentUser(id: $userId) {
                id
                __typename
              }
            }"});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("a009379fd75dbf344e170f04bca196eb6d3ba5aff06eef54b0a6129a51bd11c9");
    }

    #[test]
    fn variable_pruning_keeps_fragment_and_directive_variables() {
        let inputs = parsed_inputs(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!, $includeFriends: Boolean!, $localId: ID!) {
              localUser(id: $localId) @client
              currentUser(id: $userId) {
                ...UserFields @include(if: $includeFriends)
              }
            }

            fragment UserFields on User {
              friends(first: $userId) {
                id
              }
            }
        "});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query CurrentUserQuery($userId: ID!, $includeFriends: Boolean!) {
              currentUser(id: $userId) {
                ...UserFields @include(if: $includeFriends)
                __typename
              }
            }

            fragment UserFields on User {
              friends(first: $userId) {
                id
                __typename
              }
              __typename
            }"});
    }

    #[test]
    fn block_string_literals_match_default_typescript_manifest_formatting() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            query BlockStringQuery {
              search(text: """hello
            world""") {
                id
              }
            }
        "#});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query BlockStringQuery {
              search(text: """
              hello
              world
              """) {
                id
                __typename
              }
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("5d355c2a5cf2e2358f47521d303e0aaa4c5d5853e1b24454ed4170291b7c0a18");
    }

    #[test]
    fn export_directive_selection_sets_match_default_typescript_transform() {
        let inputs = parsed_inputs(indoc::indoc! {r#"
            query ExportQuery {
              currentUser @export(as: "currentUser") {
                id
                profile {
                  name
                }
              }
              user(id: $currentUser) {
                name
              }
            }
        "#});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations.len()).is_equal_to(1);
        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {r#"
            query ExportQuery {
              currentUser @export(as: "currentUser") {
                id
                profile {
                  name
                  __typename
                }
              }
              user(id: $currentUser) {
                name
                __typename
              }
            }"#});
        assert_that!(operations[0].id.as_str())
            .is_equal_to("235f5fc1cc144ac4e7484faf86266e6e393679e3c268b739abd3422a53adcd07");
    }

    #[test]
    fn reachable_fragments_are_sorted_by_name_and_transitive() {
        let inputs = parsed_inputs(indoc::indoc! {"
            fragment Zed on Product { z }
            fragment Alpha on Product { a ...Zed }
            query GetProduct { product { ...Alpha } }
        "});

        let operations = generate_operations(&inputs).unwrap();

        assert_that!(operations[0].body.as_str()).is_equal_to(indoc::indoc! {"
            query GetProduct {
              product {
                ...Alpha
                __typename
              }
            }

            fragment Alpha on Product {
              a
              ...Zed
              __typename
            }

            fragment Zed on Product {
              z
              __typename
            }"});
    }
}
