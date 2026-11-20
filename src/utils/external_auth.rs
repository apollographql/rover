use rover_auth::{
    external_auth::ExternalAuthError,
    external_token::{ExternalTokenOrigin, ExternalTokenSource},
};
use rover_client::RoverClientError;
use rover_print::{print::Print, style::StyledText};

/// The external authentication failure an error reports, at its root or as a client error.
///
/// `rover-client` recovers the failure from the HTTP layer once, so a caller never needs to
/// search further down the error chain.
pub(crate) fn external_auth_failure(error: &anyhow::Error) -> Option<&ExternalAuthError> {
    error.downcast_ref::<ExternalAuthError>().or_else(|| {
        match error.downcast_ref::<RoverClientError>()? {
            RoverClientError::ExternalAuth(error) => Some(error),
            _ => None,
        }
    })
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
#[error(
    "{origin} conflicts with {}. Remove the conflicting credential inputs to use the application-supplied token.",
    .inputs.join(", ")
)]
pub(crate) struct ExternalAuthConflict {
    pub(crate) origin: ExternalTokenOrigin,
    pub(crate) inputs: Vec<&'static str>,
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
