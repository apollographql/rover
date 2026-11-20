use std::fmt;

use clap::CommandFactory;
use rover_auth::{
    external_auth::ExternalAuthError,
    external_token::{ExternalTokenOrigin, ExternalTokenSource},
};
use rover_client::RoverClientError;
use rover_print::{print::Print, style::StyledText};

use crate::cli::Rover;

/// The external authentication failure an error reports, at its root or as a client error.
///
/// `rover-client` recovers the failure from the HTTP layer once, so a caller never needs to
/// search further down the error chain.
pub(crate) fn external_auth_failure(error: &anyhow::Error) -> Option<&ExternalAuthError> {
    error
        .downcast_ref::<ExternalAuthError>()
        .or_else(|| {
            error
                .downcast_ref::<ExternalAuthDiagnostic>()
                .map(|diagnostic| &diagnostic.error)
        })
        .or_else(|| match error.downcast_ref::<RoverClientError>()? {
            RoverClientError::ExternalAuth(error) => Some(error),
            _ => None,
        })
}

/// Render provenance using the same names Clap uses to parse the token-file input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalTokenOriginDisplay(String);

impl ExternalTokenOriginDisplay {
    pub(crate) fn new(origin: ExternalTokenOrigin) -> Result<Self, ExternalTokenOriginError> {
        Self::from_command(origin, &Rover::command())
    }

    fn from_command(
        origin: ExternalTokenOrigin,
        command: &clap::Command,
    ) -> Result<Self, ExternalTokenOriginError> {
        let argument = command
            .get_arguments()
            .find(|argument| argument.get_id() == "access_token_file")
            .ok_or(ExternalTokenOriginError::ArgumentNotRegistered)?;
        let name = match origin {
            ExternalTokenOrigin::Option => format!(
                "--{}",
                argument
                    .get_long()
                    .ok_or(ExternalTokenOriginError::MissingLongOption)?
            ),
            ExternalTokenOrigin::Environment => argument
                .get_env()
                .ok_or(ExternalTokenOriginError::MissingEnvironmentVariable)?
                .to_string_lossy()
                .into_owned(),
        };
        Ok(Self(name))
    }
}

impl fmt::Display for ExternalTokenOriginDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ExternalTokenOriginError {
    #[error("The external token-file argument is missing from the CLI parser.")]
    ArgumentNotRegistered,
    #[error("The external token-file argument has no long option in the CLI parser.")]
    MissingLongOption,
    #[error("The external token-file argument has no environment variable in the CLI parser.")]
    MissingEnvironmentVariable,
}

/// Keep the structured failure available while rendering CLI-specific provenance.
#[derive(Debug, thiserror::Error)]
pub(crate) struct ExternalAuthDiagnostic {
    #[source]
    error: ExternalAuthError,
    origin: Option<ExternalTokenOriginDisplay>,
}

impl ExternalAuthDiagnostic {
    pub(crate) fn new(error: ExternalAuthError) -> Result<Self, ExternalTokenOriginError> {
        Self::from_command(error, &Rover::command())
    }

    fn from_command(
        error: ExternalAuthError,
        command: &clap::Command,
    ) -> Result<Self, ExternalTokenOriginError> {
        let origin = match &error {
            ExternalAuthError::Source(error) => Some(ExternalTokenOriginDisplay::from_command(
                error.origin,
                command,
            )?),
            _ => None,
        };
        Ok(Self { error, origin })
    }
}

impl fmt::Display for ExternalAuthDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.error, &self.origin) {
            (ExternalAuthError::Source(error), Some(origin)) => {
                write!(f, "External token source from {origin}")?;
                if !error.path.as_os_str().is_empty() {
                    write!(f, " at '{}'", error.path.display())?;
                }
                write!(f, ": {}", error.kind)
            }
            (error, _) => fmt::Display::fmt(error, f),
        }
    }
}

/// Runtime selection only. Construction neither reads the file nor rejects local commands.
#[derive(Clone, Debug)]
pub(crate) struct ExternalAuthentication {
    pub(crate) source: ExternalTokenSource,
    conflicts: Vec<&'static str>,
}

impl ExternalAuthentication {
    pub(crate) const fn new(source: ExternalTokenSource, conflicts: Vec<&'static str>) -> Self {
        Self { source, conflicts }
    }

    pub(crate) fn validate(&self) -> Result<(), ExternalAuthenticationError> {
        match self.conflict()? {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn warn(&self, stderr: &impl Print) -> Result<(), ExternalTokenOriginError> {
        if let Some(conflict) = self.conflict()? {
            stderr.print(&StyledText::plain(format!(
                "Warning: {conflict} Authentication will fail when an authenticated operation starts."
            )));
        }
        Ok(())
    }

    fn conflict(&self) -> Result<Option<ExternalAuthenticationError>, ExternalTokenOriginError> {
        if self.conflicts.is_empty() {
            return Ok(None);
        }
        Ok(Some(ExternalAuthenticationError::Conflict {
            origin: ExternalTokenOriginDisplay::new(self.source.origin())?,
            inputs: self.conflicts.clone(),
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ExternalAuthenticationError {
    #[error(transparent)]
    Origin(#[from] ExternalTokenOriginError),
    #[error(
        "{origin} conflicts with {}. Remove the conflicting credential inputs to use the application-supplied token.",
        .inputs.join(", ")
    )]
    Conflict {
        origin: ExternalTokenOriginDisplay,
        inputs: Vec<&'static str>,
    },
}

#[cfg(test)]
mod tests {
    use rover_auth::{
        external_auth::ExternalAuthError,
        external_token::{ExternalTokenError, ExternalTokenFailure},
    };
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverError, RoverErrorCode};

    #[rstest]
    #[case::option(
        ExternalTokenOrigin::Option,
        "--access-token-file",
        "--application-token-file"
    )]
    #[case::environment(
        ExternalTokenOrigin::Environment,
        "APOLLO_ACCESS_TOKEN_FILE",
        "APPLICATION_TOKEN_FILE"
    )]
    fn external_auth_origin_names_follow_parser_metadata(
        #[case] origin: ExternalTokenOrigin,
        #[case] current_name: &str,
        #[case] renamed: &str,
    ) {
        let command = Rover::command().mut_arg("access_token_file", |argument| {
            argument
                .long("application-token-file")
                .env("APPLICATION_TOKEN_FILE")
        });
        assert_that!((
            ExternalTokenOriginDisplay::new(origin).unwrap().to_string(),
            ExternalTokenOriginDisplay::from_command(origin, &command)
                .unwrap()
                .to_string(),
        ))
        .is_equal_to((current_name.to_string(), renamed.to_string()));
    }

    #[rstest]
    #[case::missing_argument(
        ExternalTokenOrigin::Option,
        None,
        ExternalTokenOriginError::ArgumentNotRegistered,
        "The external token-file argument is missing from the CLI parser."
    )]
    #[case::missing_environment_argument(
        ExternalTokenOrigin::Environment,
        None,
        ExternalTokenOriginError::ArgumentNotRegistered,
        "The external token-file argument is missing from the CLI parser."
    )]
    #[case::missing_long_option(
        ExternalTokenOrigin::Option,
        Some(clap::Arg::new("access_token_file").env("APPLICATION_TOKEN_FILE")),
        ExternalTokenOriginError::MissingLongOption,
        "The external token-file argument has no long option in the CLI parser."
    )]
    #[case::missing_environment_variable(
        ExternalTokenOrigin::Environment,
        Some(clap::Arg::new("access_token_file").long("application-token-file")),
        ExternalTokenOriginError::MissingEnvironmentVariable,
        "The external token-file argument has no environment variable in the CLI parser."
    )]
    fn external_auth_missing_parser_metadata_returns_typed_errors(
        #[case] origin: ExternalTokenOrigin,
        #[case] argument: Option<clap::Arg>,
        #[case] expected: ExternalTokenOriginError,
        #[case] message: &str,
        #[values(false, true)] selection_error: bool,
    ) {
        let mut command = clap::Command::new("rover");
        if let Some(argument) = argument {
            command = command.arg(argument);
        }
        assert_that!(ExternalTokenOriginDisplay::from_command(origin, &command))
            .is_err()
            .is_equal_to(expected.clone());

        let result = ExternalAuthDiagnostic::from_command(
            ExternalAuthError::Source(ExternalTokenError {
                origin,
                path: "token-source".into(),
                kind: ExternalTokenFailure::Missing,
            }),
            &command,
        );
        let error = assert_that!(result).is_err().subject.clone();
        assert_that!(error).is_equal_to(expected);
        let error = if selection_error {
            RoverError::new(ExternalAuthenticationError::from(error))
        } else {
            RoverError::new(error)
        };
        assert_that!(error.outcome()).is_equal_to(sputnik::Outcome::InternalError);
        assert_that!(serde_json::to_value(error).unwrap()).is_equal_to(serde_json::json!({
            "code": null,
            "message": message,
        }));
    }

    #[rstest]
    #[case::option(
        ExternalTokenOrigin::Option,
        clap::Arg::new("access_token_file").long("application-token-file"),
        "--application-token-file"
    )]
    #[case::environment(
        ExternalTokenOrigin::Environment,
        clap::Arg::new("access_token_file").env("APPLICATION_TOKEN_FILE"),
        "APPLICATION_TOKEN_FILE"
    )]
    fn external_auth_origin_requires_only_its_selected_parser_metadata(
        #[case] origin: ExternalTokenOrigin,
        #[case] argument: clap::Arg,
        #[case] expected: &str,
    ) {
        let command = clap::Command::new("rover").arg(argument);
        assert_that!(
            ExternalTokenOriginDisplay::from_command(origin, &command)
                .unwrap()
                .to_string()
        )
        .is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::rejected(ExternalAuthError::Rejected)]
    #[case::permission_denied(ExternalAuthError::PermissionDenied)]
    fn external_auth_status_diagnostics_do_not_require_parser_metadata(
        #[case] error: ExternalAuthError,
    ) {
        let expected = error.to_string();
        let diagnostic =
            ExternalAuthDiagnostic::from_command(error, &clap::Command::new("rover")).unwrap();

        assert_that!(diagnostic.to_string()).is_equal_to(expected);
    }

    #[rstest]
    #[case::option_empty(
        ExternalTokenOrigin::Option,
        "",
        ExternalTokenFailure::EmptyLocation,
        RoverErrorCode::E066,
        "External token source from --access-token-file: the location is empty; supply the path to the application's token file"
    )]
    #[case::environment_empty(
        ExternalTokenOrigin::Environment,
        "",
        ExternalTokenFailure::EmptyLocation,
        RoverErrorCode::E066,
        "External token source from APOLLO_ACCESS_TOKEN_FILE: the location is empty; supply the path to the application's token file"
    )]
    #[case::option_missing(
        ExternalTokenOrigin::Option,
        "token-source",
        ExternalTokenFailure::Missing,
        RoverErrorCode::E067,
        "External token source from --access-token-file at 'token-source': the source is missing; have the supplying application publish its token file"
    )]
    #[case::environment_missing(
        ExternalTokenOrigin::Environment,
        "token-source",
        ExternalTokenFailure::Missing,
        RoverErrorCode::E067,
        "External token source from APOLLO_ACCESS_TOKEN_FILE at 'token-source': the source is missing; have the supplying application publish its token file"
    )]
    fn external_auth_source_diagnostics_keep_cli_names_through_wrappers(
        #[case] origin: ExternalTokenOrigin,
        #[case] path: &str,
        #[case] kind: ExternalTokenFailure,
        #[case] code: RoverErrorCode,
        #[case] message: &str,
        #[values(false, true)] wrapped: bool,
    ) {
        let diagnostic = ExternalAuthError::Source(ExternalTokenError {
            origin,
            path: path.into(),
            kind,
        });
        let error = if wrapped {
            anyhow::Error::new(rover_client::RoverClientError::studio_service(
                rover_graphql::GraphQLServiceError::<()>::UpstreamService(Box::new(
                    diagnostic.into_http_error(),
                )),
            ))
        } else {
            anyhow::Error::new(diagnostic)
        };
        let error = RoverError::new(error.context("Unsafe outer context: private-credential"));

        assert_that!(error.outcome()).is_equal_to(sputnik::Outcome::UserError);
        let text = temp_env::with_var("NO_COLOR", Some("1"), || error.to_string());
        assert_that!(text).is_equal_to(format!("error[{code}]: {message}\n"));
        assert_that!(serde_json::to_value(error).unwrap()).is_equal_to(serde_json::json!({
            "code": code, "message": message,
        }));
    }

    #[rstest]
    fn external_auth_conflicts_keep_their_code_through_anyhow_context() {
        let conflict = ExternalAuthenticationError::Conflict {
            origin: ExternalTokenOriginDisplay::new(ExternalTokenOrigin::Option).unwrap(),
            inputs: vec!["APOLLO_KEY"],
        };
        let error = RoverError::new(anyhow::Error::new(conflict).context("Publishing a subgraph"));

        assert_that!(serde_json::to_value(error).unwrap()).is_equal_to(serde_json::json!({
            "code": "E079",
            "message": "Publishing a subgraph",
            "causes": ["--access-token-file conflicts with APOLLO_KEY. Remove the conflicting credential inputs to use the application-supplied token."]
        }));
    }

    #[rstest]
    #[case::empty(ExternalTokenFailure::EmptyLocation, RoverErrorCode::E066)]
    #[case::missing(ExternalTokenFailure::Missing, RoverErrorCode::E067)]
    #[case::unreadable(ExternalTokenFailure::Unreadable, RoverErrorCode::E068)]
    #[case::unresolvable(ExternalTokenFailure::UnresolvableLocation, RoverErrorCode::E069)]
    #[case::oversized(ExternalTokenFailure::TooLarge, RoverErrorCode::E070)]
    #[case::invalid(ExternalTokenFailure::InvalidContents, RoverErrorCode::E071)]
    #[case::symlink(ExternalTokenFailure::SymbolicLink, RoverErrorCode::E072)]
    #[case::not_regular(ExternalTokenFailure::NotRegularFile, RoverErrorCode::E073)]
    #[case::file_owner(ExternalTokenFailure::FileOwner, RoverErrorCode::E074)]
    #[case::file_permissions(ExternalTokenFailure::FilePermissions, RoverErrorCode::E075)]
    #[case::directory_owner(ExternalTokenFailure::DirectoryOwner, RoverErrorCode::E076)]
    #[case::directory_permissions(ExternalTokenFailure::DirectoryPermissions, RoverErrorCode::E077)]
    fn external_auth_source_categories_have_distinct_stable_codes(
        #[case] kind: ExternalTokenFailure,
        #[case] code: RoverErrorCode,
    ) {
        let error = RoverError::new(ExternalAuthError::Source(ExternalTokenError {
            origin: ExternalTokenOrigin::Environment,
            path: "token-source".into(),
            kind,
        }));

        assert_that!(error.code()).is_equal_to(Some(code));
        assert_that!(error.outcome()).is_equal_to(sputnik::Outcome::UserError);
    }

    #[rstest]
    #[case::rejection(ExternalAuthError::Rejected, RoverErrorCode::E078)]
    #[case::permission(ExternalAuthError::PermissionDenied, RoverErrorCode::E080)]
    fn external_auth_error_codes_survive_transport_wrappers(
        #[case] diagnostic: ExternalAuthError,
        #[case] code: RoverErrorCode,
    ) {
        let expected_message = diagnostic.to_string();
        let error = RoverError::new(rover_client::RoverClientError::studio_service(
            rover_graphql::GraphQLServiceError::<()>::UpstreamService(Box::new(
                diagnostic.into_http_error(),
            )),
        ));

        assert_that!(serde_json::to_value(error).unwrap()).is_equal_to(serde_json::json!({
            "code": code, "message": expected_message,
        }));
    }
}
