use std::collections::HashMap;

use apollo_parser::{
    Parser,
    cst::{self, CstNode},
};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use thiserror::Error;

use crate::command::client::extensions::ExtensionSnippet;

#[derive(Debug, Error)]
pub(super) enum ParsedFileError {
    #[error("GraphQL syntax errors:\n  {}", .0.iter().join("\n  "))]
    Syntax(Vec<apollo_parser::Error>),
}

#[derive(Debug, Clone)]
pub(super) struct ParsedFile {
    pub(super) operations: Vec<OperationInput>,
    pub(super) extensions: Vec<ExtensionSnippet>,
    /// Fragment definitions keyed by name, for global deduplication across files.
    pub(super) fragments: HashMap<String, String>,
}

impl ParsedFile {
    pub(super) fn new(file: &Utf8Path, contents: &str) -> Result<Self, ParsedFileError> {
        let parser = Parser::new(contents);
        let tree = parser.parse();
        let errors: Vec<_> = tree.errors().collect();
        if !errors.is_empty() {
            let errors = errors.into_iter().cloned().collect();
            return Err(ParsedFileError::Syntax(errors));
        }

        let doc = tree.document();
        let mut extensions = Vec::new();
        let mut fragments = HashMap::new();
        for definition in doc.definitions() {
            if let cst::Definition::FragmentDefinition(ref fragment) = definition {
                let name = fragment
                    .fragment_name()
                    .and_then(|n| n.name())
                    .map(|n| n.syntax().text().to_string());
                if let Some(name) = name {
                    let range = definition.syntax().text_range();
                    let start: usize = range.start().into();
                    let end: usize = range.end().into();
                    if let Some(text) = contents.get(start..end) {
                        fragments.insert(name, text.to_string());
                    }
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
            if let cst::Definition::OperationDefinition(def) = definition
                && let Some(op) = OperationInput::new(file, contents, def)
            {
                operations.push(op);
            }
        }

        Ok(Self {
            operations,
            extensions,
            fragments,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct OperationInput {
    pub(super) name: String,
    pub(super) body: String,
    pub(super) file: Utf8PathBuf,
    pub(super) line: usize,
    pub(super) column: usize,
}

impl OperationInput {
    fn new(
        file: &Utf8Path,
        contents: &str,
        def: cst::OperationDefinition,
    ) -> Option<Self> {
        def.name().and_then(|name| {
            let range = def.syntax().text_range();
            let start: usize = range.start().into();
            let end: usize = range.end().into();
            let body = contents.get(start..end)?.to_string();

            let line = contents[..start].lines().count() + 1;
            let column = contents[..start]
                .rsplit_once('\n')
                .map(|(_, rest)| rest.len() + 1)
                .unwrap_or(1);

            Some(Self {
                name: name.syntax().text().to_string(),
                body,
                file: file.to_path_buf(),
                line,
                column,
            })
        })
    }
}
