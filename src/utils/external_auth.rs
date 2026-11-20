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
                .map(|diagnostic| &diagnostic.0)
        })
        .or_else(|| match error.downcast_ref::<RoverClientError>()? {
            RoverClientError::ExternalAuth(error) => Some(error),
            _ => None,
        })
}

/// Render provenance using the same names Clap uses to parse the token-file input.
pub(crate) struct ExternalTokenOriginDisplay<'a> {
    origin: ExternalTokenOrigin,
    argument: &'a clap::Arg,
}

impl ExternalTokenOriginDisplay<'_> {
    pub(crate) fn name(origin: ExternalTokenOrigin) -> String {
        let command = Rover::command();
        let argument = command
            .get_arguments()
            .find(|argument| argument.get_id() == "access_token_file")
            .expect("the token-file argument is registered");
        ExternalTokenOriginDisplay { origin, argument }.to_string()
    }
}

impl fmt::Display for ExternalTokenOriginDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.origin {
            ExternalTokenOrigin::Option => write!(
                f,
                "--{}",
                self.argument
                    .get_long()
                    .expect("the token-file argument has a long option")
            ),
            ExternalTokenOrigin::Environment => f.write_str(
                &self
                    .argument
                    .get_env()
                    .expect("the token-file argument has an environment variable")
                    .to_string_lossy(),
            ),
        }
    }
}

/// Keep the structured failure available while rendering CLI-specific provenance.
#[derive(Debug, thiserror::Error)]
pub(crate) struct ExternalAuthDiagnostic(#[source] pub(crate) ExternalAuthError);

impl fmt::Display for ExternalAuthDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ExternalAuthError::Source(error) => {
                write!(
                    f,
                    "External token source from {}",
                    ExternalTokenOriginDisplay::name(error.origin)
                )?;
                if !error.path.as_os_str().is_empty() {
                    write!(f, " at '{}'", error.path.display())?;
                }
                write!(f, ": {}", error.kind)
            }
            error => fmt::Display::fmt(error, f),
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

    pub(crate) fn validate(&self) -> Result<(), ExternalAuthConflict> {
        match self.conflict() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn warn(&self, stderr: &impl Print) {
        if let Some(conflict) = self.conflict() {
            stderr.print(&StyledText::plain(format!(
                "Warning: {conflict} Authentication will fail when an authenticated operation starts."
            )));
        }
    }

    fn conflict(&self) -> Option<ExternalAuthConflict> {
        (!self.conflicts.is_empty()).then(|| ExternalAuthConflict {
            origin: self.source.origin(),
            inputs: self.conflicts.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) struct ExternalAuthConflict {
    pub(crate) origin: ExternalTokenOrigin,
    pub(crate) inputs: Vec<&'static str>,
}

impl fmt::Display for ExternalAuthConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} conflicts with {}. Remove the conflicting credential inputs to use the application-supplied token.",
            ExternalTokenOriginDisplay::name(self.origin),
            self.inputs.join(", ")
        )
    }
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
        let argument = command
            .get_arguments()
            .find(|argument| argument.get_id() == "access_token_file")
            .unwrap();

        assert_that!((
            ExternalTokenOriginDisplay::name(origin),
            ExternalTokenOriginDisplay { origin, argument }.to_string(),
        ))
        .is_equal_to((current_name.to_string(), renamed.to_string()));
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
        let conflict = ExternalAuthConflict {
            origin: ExternalTokenOrigin::Option,
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
