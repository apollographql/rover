use std::collections::{BTreeMap, HashMap};

use apollo_compiler::{ast, parser::Parser as ApolloParser};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use rover_std::{Fs, sha256_hex};

use super::{
    parsed_fragment::ParsedFragment, parsed_operation::ParsedOperation,
    persisted_query_operation::PersistedQueryOperation,
};
use crate::command::persisted_queries::generate::manifest::{
    ast_ext::SelectionSetExt,
    error::{GenerateError, ParseFailure},
};

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

    pub(crate) fn from_files(files: Vec<Utf8PathBuf>) -> Result<Self, GenerateError> {
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

    pub(super) fn merge(&mut self, other: ParsedInputs) -> Result<(), GenerateError> {
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

    pub(crate) fn generate_operations(
        &self,
    ) -> Result<Vec<PersistedQueryOperation>, GenerateError> {
        self.generate_operations_with_id(|s| sha256_hex(s))
    }

    pub(super) fn generate_operations_with_id(
        &self,
        id_fn: impl Fn(&str) -> String,
    ) -> Result<Vec<PersistedQueryOperation>, GenerateError> {
        let mut operation_ids = HashMap::new();
        self.operations
            .iter()
            .map(|(name, operation)| {
                let body = operation.body(name, &self.fragments)?;
                let id = id_fn(&body);
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

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use speculoos::prelude::*;

    use super::ParsedInputs;

    #[test]
    fn anonymous_operation_in_file_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let file = Utf8PathBuf::from_path_buf(temp.path().join("ops.graphql")).unwrap();
        std::fs::write(&file, "query { field }").unwrap();
        let result = ParsedInputs::from_file(&file).map_err(|e| e.to_string());

        assert_that!(result).is_err().is_equal_to(format!(
            "{file}: Anonymous GraphQL operations are not supported. Please name your query."
        ));
    }

    #[test]
    fn duplicate_operation_in_same_file_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let file = Utf8PathBuf::from_path_buf(temp.path().join("ops.graphql")).unwrap();
        std::fs::write(&file, "query GetUser { id }\nquery GetUser { name }").unwrap();
        let result = ParsedInputs::from_file(&file).map_err(|e| e.to_string());

        assert_that!(result).is_err().is_equal_to(format!(
            "{file}: Operation named \"GetUser\" is already defined in {file}. Duplicate found in {file}."
        ));
    }

    #[test]
    fn duplicate_operation_across_files_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let a = Utf8PathBuf::from_path_buf(temp.path().join("a.graphql")).unwrap();
        let b = Utf8PathBuf::from_path_buf(temp.path().join("b.graphql")).unwrap();
        std::fs::write(&a, "query GetUser { id }").unwrap();
        std::fs::write(&b, "query GetUser { name }").unwrap();

        let mut combined = ParsedInputs::default();
        combined
            .merge(ParsedInputs::from_file(&a).unwrap())
            .unwrap();
        let result = combined
            .merge(ParsedInputs::from_file(&b).unwrap())
            .map_err(|e| e.to_string());

        assert_that!(result).is_err().is_equal_to(format!(
            "Operation named \"GetUser\" is already defined in {a}. Duplicate found in {b}."
        ));
    }

    #[test]
    fn multiple_files_with_parse_errors_returns_all_failures() {
        let temp = tempfile::tempdir().unwrap();
        let a = Utf8PathBuf::from_path_buf(temp.path().join("a.graphql")).unwrap();
        let b = Utf8PathBuf::from_path_buf(temp.path().join("b.graphql")).unwrap();
        std::fs::write(&a, "query { field }").unwrap();
        std::fs::write(&b, "query { other }").unwrap();

        let result = ParsedInputs::from_files(vec![a.clone(), b.clone()]);

        assert_that!(result).is_err();
        let msg = result.unwrap_err().to_string();
        assert_that!(msg).contains(a.as_str());
        assert_that!(msg).contains(b.as_str());
    }

    #[test]
    fn duplicate_fragment_across_files_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let a = Utf8PathBuf::from_path_buf(temp.path().join("a.graphql")).unwrap();
        let b = Utf8PathBuf::from_path_buf(temp.path().join("b.graphql")).unwrap();
        std::fs::write(&a, "fragment F on T { id }").unwrap();
        std::fs::write(&b, "fragment F on T { name }").unwrap();

        let mut combined = ParsedInputs::default();
        combined
            .merge(ParsedInputs::from_file(&a).unwrap())
            .unwrap();
        let result = combined
            .merge(ParsedInputs::from_file(&b).unwrap())
            .map_err(|e| e.to_string());

        assert_that!(result).is_err().is_equal_to(format!(
            "Fragment named \"F\" is already defined in {a}. Duplicate found in {b}."
        ));
    }
}
