use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use apollo_compiler::{Node, ast, name, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use itertools::Itertools;
use rover_std::{FileSearch, Fs, Style};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{RoverError, RoverOutput, RoverResult};

const DEFAULT_INCLUDE: &str = "graphql/**/*.graphql";
const DEFAULT_OUTPUT: &str = "persisted-query-manifest.json";
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

enum PrintableDefinition {
    Operation {
        operation: Node<ast::OperationDefinition>,
        source: Arc<str>,
    },
    Fragment {
        fragment: Node<ast::FragmentDefinition>,
        source: Arc<str>,
    },
}

impl Generate {
    pub async fn run(&self, output_file: Option<Utf8PathBuf>) -> RoverResult<RoverOutput> {
        let manifest = self.generate_manifest()?;
        let output_path = resolve_output_path(output_file)?;
        let operation_count = manifest.operations.len();
        let manifest_json = format!("{}\n", serde_json::to_string_pretty(&manifest)?);

        if operation_count == 0 {
            eprintln!(
                "{} no operations found during manifest generation. You may need to adjust the glob pattern used to search files in this project.",
                Style::WarningPrefix.paint("warning:"),
            );
        }
        Fs::write_file(&output_path, manifest_json)?;
        eprintln!(
            "{} Manifest written to {} with {} operation{}.",
            Style::Success.paint("Success:"),
            Style::Path.paint(&output_path),
            operation_count,
            if operation_count == 1 { "" } else { "s" }
        );

        Ok(RoverOutput::OutputHandled)
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
            .map_err(RoverError::from)
    }
}

fn normalize_includes(includes: &[String], canonical_root: &std::path::Path) -> Vec<String> {
    includes
        .iter()
        .map(|p| {
            let path = std::path::Path::new(p);
            if path.is_absolute() {
                let canonical = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
                canonical
                    .strip_prefix(canonical_root)
                    .map(|rel| rel.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| p.clone())
            } else {
                p.clone()
            }
        })
        .collect()
}

fn resolve_output_path(output_file: Option<Utf8PathBuf>) -> RoverResult<Utf8PathBuf> {
    match output_file {
        Some(path) => Ok(path),
        None => {
            let cwd = std::env::current_dir()?;
            Utf8PathBuf::from_path_buf(cwd)
                .map(|cwd| cwd.join(DEFAULT_OUTPUT))
                .map_err(|_| GenerateError::NonUtf8CurrentDir.into())
        }
    }
}

fn parse_files(files: Vec<Utf8PathBuf>) -> RoverResult<ParsedInputs> {
    let mut inputs = ParsedInputs::default();
    let mut parse_failures = Vec::new();

    for file in files {
        match parse_file(&file) {
            Ok(parsed_file) => merge_parsed_file(&mut inputs, parsed_file)?,
            Err(failure) => parse_failures.push(failure),
        }
    }

    if !parse_failures.is_empty() {
        Err(GenerateError::ParseFailures { parse_failures })?;
    }

    Ok(inputs)
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
                operation_type: operation_type_json(operation.operation.operation_type),
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
    let reachable_fragments = reachable_fragments(
        operation_name,
        &operation.direct_fragment_spreads,
        fragments,
    )?;
    let mut operation_node = operation.operation.clone();
    add_typename_to_operation(operation_node.make_mut());

    let mut fragment_definitions = Vec::new();
    for fragment_name in reachable_fragments {
        let fragment = fragments
            .get(&fragment_name)
            .expect("reachable fragments are validated before returning");
        let mut fragment_node = fragment.fragment.clone();
        let fragment_definition = fragment_node.make_mut();
        fragment_definition
            .directives
            .0
            .retain(|directive| directive.name != "client");
        add_typename_to_selection_set(&mut fragment_definition.selection_set);
        fragment_definitions.push((fragment_node, Arc::clone(&fragment.source)));
    }

    prune_unused_variables(operation_node.make_mut(), &fragment_definitions);

    let mut definitions = vec![PrintableDefinition::Operation {
        operation: operation_node,
        source: Arc::clone(&operation.source),
    }];
    definitions.extend(
        fragment_definitions
            .into_iter()
            .map(|(fragment, source)| PrintableDefinition::Fragment { fragment, source }),
    );

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
    let mut spreads = BTreeSet::new();
    for selection in selections {
        if selection_has_directive(selection, "client") {
            continue;
        }
        match selection {
            ast::Selection::FragmentSpread(fragment_spread) => {
                spreads.insert(fragment_spread.fragment_name.to_string());
            }
            ast::Selection::Field(field) => {
                spreads.extend(collect_spreads(&field.selection_set));
            }
            ast::Selection::InlineFragment(inline_fragment) => {
                spreads.extend(collect_spreads(&inline_fragment.selection_set));
            }
        }
    }
    spreads
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
            name: name!("__typename"),
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
    let mut used_variables = BTreeSet::new();
    collect_variables_from_operation(operation, &mut used_variables);
    for (fragment, _) in fragments {
        collect_variables_from_fragment(fragment, &mut used_variables);
    }
    operation
        .variables
        .retain(|variable| used_variables.contains(variable.name.as_str()));
}

fn collect_variables_from_operation(
    operation: &ast::OperationDefinition,
    variables: &mut BTreeSet<String>,
) {
    collect_variables_from_directives(&operation.directives, variables);
    collect_variables_from_selection_set(&operation.selection_set, variables);
}

fn collect_variables_from_fragment(
    fragment: &ast::FragmentDefinition,
    variables: &mut BTreeSet<String>,
) {
    collect_variables_from_directives(&fragment.directives, variables);
    collect_variables_from_selection_set(&fragment.selection_set, variables);
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

fn print_document(definitions: &[PrintableDefinition]) -> String {
    definitions
        .iter()
        .map(print_definition)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn print_definition(definition: &PrintableDefinition) -> String {
    let mut output = String::new();
    match definition {
        PrintableDefinition::Operation { operation, source } => {
            print_operation(&mut output, operation, source)
        }
        PrintableDefinition::Fragment { fragment, source } => {
            print_fragment(&mut output, fragment, source)
        }
    }
    output
}

fn print_operation(output: &mut String, operation: &ast::OperationDefinition, source: &str) {
    output.push_str(operation_type_json(operation.operation_type));
    if let Some(name) = &operation.name {
        output.push(' ');
        output.push_str(name);
    }
    print_variable_definitions(output, &operation.variables, source);
    print_directives(output, &operation.directives, source, 0);
    output.push(' ');
    print_selection_set(output, &operation.selection_set, source, 0);
}

fn print_fragment(output: &mut String, fragment: &ast::FragmentDefinition, source: &str) {
    output.push_str("fragment ");
    output.push_str(&fragment.name);
    output.push_str(" on ");
    output.push_str(&fragment.type_condition);
    print_directives(output, &fragment.directives, source, 0);
    output.push(' ');
    print_selection_set(output, &fragment.selection_set, source, 0);
}

fn print_selection_set(
    output: &mut String,
    selections: &[ast::Selection],
    source: &str,
    indent: usize,
) {
    output.push_str("{\n");
    for selection in selections {
        push_indent(output, indent + 2);
        print_selection(output, selection, source, indent + 2);
        output.push('\n');
    }
    push_indent(output, indent);
    output.push('}');
}

fn print_selection(output: &mut String, selection: &ast::Selection, source: &str, indent: usize) {
    match selection {
        ast::Selection::Field(field) => print_field(output, field, source, indent),
        ast::Selection::FragmentSpread(fragment_spread) => {
            output.push_str("...");
            output.push_str(&fragment_spread.fragment_name);
            print_directives(output, &fragment_spread.directives, source, indent);
        }
        ast::Selection::InlineFragment(inline_fragment) => {
            output.push_str("...");
            if let Some(type_condition) = &inline_fragment.type_condition {
                output.push_str(" on ");
                output.push_str(type_condition);
            }
            print_directives(output, &inline_fragment.directives, source, indent);
            output.push(' ');
            print_selection_set(output, &inline_fragment.selection_set, source, indent);
        }
    }
}

fn print_field(output: &mut String, field: &ast::Field, source: &str, indent: usize) {
    if let Some(alias) = &field.alias {
        output.push_str(alias);
        output.push_str(": ");
    }
    output.push_str(&field.name);
    print_arguments(output, &field.arguments, source, indent);
    print_directives(output, &field.directives, source, indent);
    if !field.selection_set.is_empty() {
        output.push(' ');
        print_selection_set(output, &field.selection_set, source, indent);
    }
}

fn print_variable_definitions(
    output: &mut String,
    variables: &[Node<ast::VariableDefinition>],
    source: &str,
) {
    if variables.is_empty() {
        return;
    }

    output.push('(');
    for (idx, variable) in variables.iter().enumerate() {
        if idx > 0 {
            output.push_str(", ");
        }
        output.push('$');
        output.push_str(&variable.name);
        output.push_str(": ");
        print_type(output, &variable.ty);
        if let Some(default_value) = &variable.default_value {
            output.push_str(" = ");
            print_value(output, default_value, source, 0);
        }
        print_directives(output, &variable.directives, source, 0);
    }
    output.push(')');
}

fn print_type(output: &mut String, ty: &ast::Type) {
    match ty {
        ast::Type::Named(name) => output.push_str(name),
        ast::Type::NonNullNamed(name) => {
            output.push_str(name);
            output.push('!');
        }
        ast::Type::List(inner) => {
            output.push('[');
            print_type(output, inner);
            output.push(']');
        }
        ast::Type::NonNullList(inner) => {
            output.push('[');
            print_type(output, inner);
            output.push_str("]!");
        }
    }
}

fn print_directives(
    output: &mut String,
    directives: &ast::DirectiveList,
    source: &str,
    indent: usize,
) {
    for directive in directives.iter() {
        output.push(' ');
        output.push('@');
        output.push_str(&directive.name);
        print_arguments(output, &directive.arguments, source, indent);
    }
}

fn print_arguments(
    output: &mut String,
    arguments: &[Node<ast::Argument>],
    source: &str,
    indent: usize,
) {
    if arguments.is_empty() {
        return;
    }

    output.push('(');
    for (idx, argument) in arguments.iter().enumerate() {
        if idx > 0 {
            output.push_str(", ");
        }
        output.push_str(&argument.name);
        output.push_str(": ");
        print_value(output, &argument.value, source, indent);
    }
    output.push(')');
}

fn print_value(output: &mut String, value_node: &Node<ast::Value>, source: &str, indent: usize) {
    match value_node.as_ref() {
        ast::Value::Null => output.push_str("null"),
        ast::Value::Enum(name) | ast::Value::Variable(name) => {
            if matches!(value_node.as_ref(), ast::Value::Variable(_)) {
                output.push('$');
            }
            output.push_str(name);
        }
        ast::Value::String(value) => {
            if value_was_block_string(value_node, source) {
                print_block_string(output, value, indent);
            } else {
                output.push_str(
                    &ast::Value::String(value.clone())
                        .serialize()
                        .no_indent()
                        .to_string(),
                );
            }
        }
        ast::Value::Float(value) => output.push_str(&value.to_string()),
        ast::Value::Int(value) => output.push_str(&value.to_string()),
        ast::Value::Boolean(value) => output.push_str(if *value { "true" } else { "false" }),
        ast::Value::List(values) => {
            output.push('[');
            for (idx, value) in values.iter().enumerate() {
                if idx > 0 {
                    output.push_str(", ");
                }
                print_value(output, value, source, indent);
            }
            output.push(']');
        }
        ast::Value::Object(fields) => {
            output.push('{');
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
                    output.push_str(", ");
                }
                output.push_str(name);
                output.push_str(": ");
                print_value(output, value, source, indent);
            }
            output.push('}');
        }
    }
}

fn value_was_block_string(value: &Node<ast::Value>, source: &str) -> bool {
    value
        .location()
        .and_then(|location| source.get(location.offset()..location.end_offset()))
        .is_some_and(|source_text| source_text.trim_start().starts_with("\"\"\""))
}

fn print_block_string(output: &mut String, value: &str, indent: usize) {
    output.push_str("\"\"\"");
    output.push('\n');
    for line in value.split('\n') {
        push_indent(output, indent);
        output.push_str(&line.replace("\"\"\"", "\\\"\"\""));
        output.push('\n');
    }
    push_indent(output, indent);
    output.push_str("\"\"\"");
}

fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push(' ');
    }
}

const fn operation_type_json(operation_type: ast::OperationType) -> &'static str {
    match operation_type {
        ast::OperationType::Query => "query",
        ast::OperationType::Mutation => "mutation",
        ast::OperationType::Subscription => "subscription",
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
