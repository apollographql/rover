use camino::Utf8PathBuf;
use itertools::Itertools;

#[derive(Debug, thiserror::Error)]
pub(crate) enum GenerateError {
    #[error("current directory is not utf-8")]
    NonUtf8CurrentDir,
    #[error("Failed to parse {} .graphql file(s):\n{}", .parse_failures.len(), .parse_failures.iter().join("\n"))]
    ParseFailures { parse_failures: Vec<ParseFailure> },
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
pub(super) struct ParseFailure {
    pub(super) file: Utf8PathBuf,
    pub(super) message: String,
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn anonymous_operation_error_message_names_the_file_and_operation_type() {
        let err = GenerateError::AnonymousOperation {
            file: "ops.graphql".into(),
            operation_type: "query".to_string(),
        };
        assert_that!(err.to_string()).is_equal_to(
            "Anonymous GraphQL operations are not supported. Please name your query in ops.graphql."
                .to_string(),
        );
    }

    #[test]
    fn duplicate_operation_error_message_names_both_files() {
        let err = GenerateError::DuplicateOperation {
            name: "GetUser".to_string(),
            first_file: "a.graphql".into(),
            second_file: "b.graphql".into(),
        };
        assert_that!(err.to_string()).is_equal_to(
            r#"Operation named "GetUser" is already defined in a.graphql. Duplicate found in b.graphql."#
                .to_string(),
        );
    }

    #[test]
    fn duplicate_fragment_error_message_names_both_files() {
        let err = GenerateError::DuplicateFragment {
            name: "UserFields".to_string(),
            first_file: "a.graphql".into(),
            second_file: "b.graphql".into(),
        };
        assert_that!(err.to_string()).is_equal_to(
            r#"Fragment named "UserFields" is already defined in a.graphql. Duplicate found in b.graphql."#
                .to_string(),
        );
    }

    #[test]
    fn missing_fragment_error_message_names_operation_and_fragment() {
        let err = GenerateError::MissingFragment {
            operation_name: "GetUser".to_string(),
            fragment_name: "UserFields".to_string(),
        };
        assert_that!(err.to_string()).is_equal_to(
            r#"Operation named "GetUser" references missing fragment "UserFields"."#.to_string(),
        );
    }

    #[test]
    fn duplicate_operation_id_error_message_names_both_operations() {
        let err = GenerateError::DuplicateOperationId {
            id: "abc123".to_string(),
            operation_name: "GetUser".to_string(),
            existing_operation_name: "FetchUser".to_string(),
        };
        assert_that!(err.to_string()).is_equal_to(
            r#"Generated operation ID abc123 for operation "GetUser" was already used for operation "FetchUser"."#
                .to_string(),
        );
    }

    #[test]
    fn generate_failure_display_includes_file_and_message() {
        let err = ParseFailure {
            file: "ops.graphql".into(),
            message: "syntax error".to_string(),
        };
        assert_that!(err.to_string()).is_equal_to("ops.graphql: syntax error".to_string());
    }
}
