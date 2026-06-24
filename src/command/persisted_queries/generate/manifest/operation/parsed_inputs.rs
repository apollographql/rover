use std::collections::{BTreeMap, HashMap};

use apollo_compiler::{ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use rover_std::Fs;

use super::{
    parsed_fragment::ParsedFragment,
    parsed_operation::ParsedOperation,
    persisted_query_operation::{PersistedQueryOperation, sha256_hex},
};
use super::super::{
    ast_ext::SelectionSetExt,
    error::{GenerateError, ParseFailure},
};
use crate::RoverResult;

#[derive(Debug, Default)]
pub(crate) struct ParsedInputs {
    pub(super) operations: BTreeMap<String, ParsedOperation>,
    pub(super) fragments: BTreeMap<String, ParsedFragment>,
}

impl ParsedInputs {
    pub(super) fn from_file(file: &Utf8Path) -> Result<Self, ParseFailure> {
        let contents = Fs::read_file(file).map_err(|err| ParseFailure {
            file: file.to_path_buf(),
            message: err.to_string(),
        })?;
        let document = ApolloParser::new()
            .parse_ast(contents, file.as_std_path())
            .map_err(|err| ParseFailure {
                file: file.to_path_buf(),
                message: err.to_string(),
            })?;

        let mut parsed = Self::default();
        for definition in document.definitions {
            match definition {
                ast::Definition::OperationDefinition(operation) => {
                    let name = operation
                        .name
                        .as_ref()
                        .map(ToString::to_string)
                        .ok_or_else(|| ParseFailure {
                            file: file.to_path_buf(),
                            message: GenerateError::AnonymousOperation {
                                file: file.to_path_buf(),
                                operation_type: operation.operation_type.to_string(),
                            }
                            .to_string(),
                        })?;
                    if parsed.operations.contains_key(&name) {
                        return Err(ParseFailure {
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
                            direct_fragment_spreads: operation.selection_set.collect_spreads(),
                            operation,
                        },
                    );
                }
                ast::Definition::FragmentDefinition(fragment) => {
                    if parsed.fragments.contains_key(fragment.name.as_str()) {
                        return Err(ParseFailure {
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
                            direct_fragment_spreads: fragment.selection_set.collect_spreads(),
                            fragment,
                        },
                    );
                }
                _ => {}
            }
        }
        Ok(parsed)
    }

    pub(crate) fn from_files(files: Vec<Utf8PathBuf>) -> RoverResult<Self> {
        let (parsed, failures): (Vec<_>, Vec<_>) = files
            .into_iter()
            .map(|file| Self::from_file(&file))
            .partition_result();

        if !failures.is_empty() {
            Err(GenerateError::ParseFailures {
                parse_failures: failures,
            })?;
        }

        parsed
            .into_iter()
            .try_fold(Self::default(), |mut acc, file| {
                acc.merge(file)?;
                Ok(acc)
            })
    }

    pub(super) fn merge(&mut self, other: ParsedInputs) -> RoverResult<()> {
        for (name, operation) in other.operations {
            if let Some(existing) = self.operations.get(&name) {
                Err(GenerateError::DuplicateOperation {
                    name: name.clone(),
                    first_file: existing.file.clone(),
                    second_file: operation.file.clone(),
                })?;
            }
            self.operations.insert(name, operation);
        }

        for (name, fragment) in other.fragments {
            if let Some(existing) = self.fragments.get(&name) {
                Err(GenerateError::DuplicateFragment {
                    name: name.clone(),
                    first_file: existing.file.clone(),
                    second_file: fragment.file.clone(),
                })?;
            }
            self.fragments.insert(name, fragment);
        }

        Ok(())
    }

    pub(crate) fn generate_operations(&self) -> RoverResult<Vec<PersistedQueryOperation>> {
        let mut operation_ids = HashMap::new();
        self.operations
            .iter()
            .map(|(name, operation)| {
                let body = operation.body(name, &self.fragments)?;
                let id = sha256_hex(&body);
                if let Some(existing_operation_name) =
                    operation_ids.insert(id.clone(), name.clone())
                {
                    Err(GenerateError::DuplicateOperationId {
                        id: id.clone(),
                        operation_name: name.clone(),
                        existing_operation_name,
                    })?;
                }
                Ok(PersistedQueryOperation {
                    id,
                    name: name.clone(),
                    operation_type: operation.operation.operation_type.name(),
                    body,
                })
            })
            .collect()
    }
}
