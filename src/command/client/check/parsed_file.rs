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
        let mut fragment_text = Vec::new();
        for definition in doc.definitions() {
            if let cst::Definition::FragmentDefinition(fragment) = definition {
                let range = fragment.syntax().text_range();
                let start: usize = range.start().into();
                let end: usize = range.end().into();
                if let Some(text) = contents.get(start..end) {
                    fragment_text.push(text.to_string());
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
                && let Some(op) = OperationInput::new(file, contents, def, &fragment_text)
            {
                operations.push(op);
            }
        }

        Ok(Self {
            operations,
            extensions,
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
        fragments: &[String],
    ) -> Option<Self> {
        def.name().and_then(|name| {
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
