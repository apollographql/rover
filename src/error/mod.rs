mod metadata;

pub use metadata::{RoverErrorCode, RoverErrorMetadata, RoverErrorSuggestion};

pub type RoverResult<T> = std::result::Result<T, RoverError>;

use std::{
    borrow::BorrowMut,
    error::Error,
    fmt::{self, Debug, Display},
};

use apollo_federation_types::rover::BuildErrors;
use calm_io::{stderr, stdoutln};
use rover_client::RoverClientError;
use rover_std::Style;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Value, json};

use crate::{command::CliOutput, options::JsonVersion, plugin::error::PluginFailure};

/// A specialized `Error` type for Rover that wraps `anyhow`
/// and provides some extra `Metadata` for end users depending
/// on the specific error they encountered.
#[derive(Debug)]
pub struct RoverError {
    error: anyhow::Error,
    metadata: RoverErrorMetadata,
}

impl Serialize for RoverError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut data = serializer.serialize_struct("error", 3)?;
        data.serialize_field("message", &self.error.to_string())?;

        // `BuildErrors` already carry their own structured detail, so surface that directly.
        // Otherwise, surface the rest of the `anyhow` cause chain (the "Caused by:" lines shown
        // in plain-text output) so that `--format json` consumers get the same file-level detail
        // — e.g. which schema file could not be found during `supergraph compose`. We respect
        // `skip_printing_cause` for parity with the `Display` impl: some errors (e.g. reqwest)
        // already fold their cause into the top-level message.
        if let Some(build_errors) = self
            .error
            .downcast_ref::<RoverClientError>()
            .and_then(Error::source)
            .and_then(|source| source.downcast_ref::<BuildErrors>())
        {
            data.serialize_field("details", build_errors)?;
        } else if !self.metadata.skip_printing_cause {
            let causes: Vec<String> = self
                .error
                .chain()
                .skip(1)
                .map(ToString::to_string)
                .collect();
            if !causes.is_empty() {
                data.serialize_field("causes", &causes)?;
            }
        }

        data.serialize_field("code", &self.metadata.code)?;
        data.end()
    }
}

impl RoverError {
    pub fn new<E>(error: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let mut error = error.into();
        let metadata = RoverErrorMetadata::from(error.borrow_mut());

        Self { error, metadata }
    }

    pub fn set_suggestion(&mut self, suggestion: RoverErrorSuggestion) {
        self.metadata.suggestions.push(suggestion);
    }

    pub fn with_suggestion(mut self, suggestion: RoverErrorSuggestion) -> Self {
        self.set_suggestion(suggestion);
        self
    }

    pub fn suggestions(&self) -> &[RoverErrorSuggestion] {
        &self.metadata.suggestions
    }

    pub fn message(&self) -> String {
        self.error.to_string()
    }

    pub fn code(&self) -> Option<RoverErrorCode> {
        self.metadata.code.clone()
    }

    /// The plugin failure behind this error, if there is one, so that a
    /// caller converting it into its own error type can keep it.
    pub(crate) fn plugin_failure(&self) -> Option<&PluginFailure> {
        crate::plugin::error::find_in_chain(&self.error)
    }

    pub fn print(&self) -> RoverResult<()> {
        match self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response,
            }) => stdoutln!(
                "{}",
                crate::command::check_output::CheckWorkflowOutput(check_response).text()
            )?,
            Some(RoverClientError::LintFailures { lint_response }) => {
                stdoutln!("{}", lint_response.get_ariadne()?)?
            }
            _ => (),
        }

        stderr!("{}", self)?;
        Ok(())
    }

    pub(crate) fn get_internal_data_json(&self) -> Value {
        // A run that failed still reports the plugin it used (FR57): the first
        // question asked of a composition failure is which federation version
        // rejected the schema, and `--format json` has no other field carrying
        // it — `error` renders the plugin's identity only inside its prose.
        //
        // Read by downcast rather than by a field on `RoverError`, so this stays
        // an extension point three errors already use rather than a new
        // obligation on all 112 construction sites.
        //
        // Empty rather than absent when nothing resolved, because FR57's
        // presence guarantee is what lets a consumer tell "this run used no
        // plugin" from "this Rover predates the field" — and only a plugin-using
        // command reaches here, so the field is never bolted onto errors from
        // commands that have no plugins to report.
        #[cfg(feature = "composition-js")]
        {
            use crate::composition::{CompositionError, supergraph::binary::BinaryError};

            if let Some(error) = self.error.downcast_ref::<CompositionError>() {
                return json!({ "plugins": error.provenance().into_iter().collect::<Vec<_>>() });
            }

            if let Some(error) = self.error.downcast_ref::<BinaryError>() {
                return json!({ "plugins": [error.provenance()] });
            }

            // Resolution never got as far as a plugin — a missing or malformed
            // `supergraph.yaml` is the commonest way a plugin-using command
            // fails, and it has to report an empty array rather than no field
            // for the guarantee above to be worth anything.
            if self
                .error
                .downcast_ref::<crate::composition::pipeline::CompositionPipelineError>()
                .is_some()
            {
                return json!({ "plugins": [] });
            }
        }

        match self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response,
            }) => check_response.get_json(),
            Some(RoverClientError::LintFailures { lint_response }) => lint_response.get_json(),
            Some(RoverClientError::PublishLaunchFailure {
                graph_ref: _,
                publish_response,
            }) => publish_response.clone(),
            _ => Value::Null,
        }
    }

    pub(crate) fn get_internal_error_json(&self) -> Value {
        #[cfg(feature = "composition-js")]
        {
            use crate::composition::CompositionError;
            match self.error.downcast_ref::<CompositionError>() {
                Some(CompositionError::Build { source, .. }) => {
                    json!({"details": source, "code": self.code(), "message": self.message()})
                }
                _ => json!(self),
            }
        }
        #[cfg(not(feature = "composition-js"))]
        json!(self)
    }

    pub(crate) fn get_json_version(&self) -> JsonVersion {
        match &self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response: _,
            }) => JsonVersion::Three,
            _ => self.metadata.json_version.clone(),
        }
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_descriptor_message = if let Some(code) = &self.metadata.code {
            format!("error[{code}]:")
        } else {
            "error:".to_string()
        };
        let error_descriptor = Style::ErrorPrefix.paint(error_descriptor_message);

        if self.metadata.skip_printing_cause {
            writeln!(formatter, "{} {}", error_descriptor, self.error)?;
        } else {
            writeln!(formatter, "{} {:?}", error_descriptor, self.error)?;
        }

        for suggestion in &self.metadata.suggestions {
            writeln!(formatter, "        {suggestion}")?;
        }
        Ok(())
    }
}

impl<E: Into<anyhow::Error>> From<E> for RoverError {
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use serde_json::json;

    use super::RoverError;

    #[test]
    fn json_output_surfaces_the_anyhow_cause_chain() {
        // Mirrors the `supergraph compose` file-not-found case: the io error is the outermost
        // message and "could not find '<path>'" is the cause (see rover_std::Fs::read_file).
        let error = anyhow!("could not find '/path/to/schema.graphql'")
            .context("No such file or directory (os error 2)");
        let value = serde_json::to_value(RoverError::new(error)).unwrap();

        assert_eq!(
            value["message"],
            json!("No such file or directory (os error 2)")
        );
        assert_eq!(
            value["causes"],
            json!(["could not find '/path/to/schema.graphql'"])
        );
    }

    #[test]
    fn json_output_omits_causes_for_a_single_error() {
        let value = serde_json::to_value(RoverError::new(anyhow!("a flat error"))).unwrap();

        assert_eq!(value["message"], json!("a flat error"));
        assert!(value.get("causes").is_none());
    }

    #[cfg(feature = "composition-js")]
    mod plugins_on_the_failure_path {
        use apollo_federation_types::{
            config::FederationVersion,
            rover::{BuildError, BuildErrors},
        };
        use camino::Utf8PathBuf;
        use rstest::rstest;
        use semver::Version;
        use serde_json::json;
        use speculoos::prelude::*;

        use super::RoverError;
        use crate::{
            command::install::{PluginLevel, PluginProvenance, PluginSource},
            composition::{
                CompositionError, pipeline::CompositionPipelineError,
                supergraph::binary::BinaryError,
            },
            options::JsonOutput,
        };

        /// `BuildErrors` renders its own length, so it has to be non-empty.
        fn build_errors() -> BuildErrors {
            BuildErrors::from(vec![BuildError::composition_error(
                Some("E029".to_string()),
                Some("a field is not resolvable".to_string()),
                None,
                None,
            )])
        }

        fn provenance() -> PluginProvenance {
            PluginProvenance::new(
                "supergraph",
                Version::new(2, 9, 3),
                PluginSource::Downloaded,
                PluginLevel::Global,
                Utf8PathBuf::from("/plugins/supergraph-v2.9.3"),
            )
        }

        /// The entry a successful run reports under `data.plugins`. A failure
        /// has to report the same shape or a consumer needs two readers.
        fn expected_entry() -> serde_json::Value {
            json!({
                "name": "supergraph",
                "version": "2.9.3",
                "source": "downloaded",
                "level": "global",
                "path": "/plugins/supergraph-v2.9.3",
            })
        }

        /// Walks the real path — through `RoverError` and the JSON envelope —
        /// rather than calling the accessor, since the envelope is the contract.
        fn data_of(error: impl Into<anyhow::Error>) -> serde_json::Value {
            let rover_error = RoverError::new(error.into());
            let envelope = serde_json::to_value(JsonOutput::from(&rover_error)).unwrap();
            envelope["data"].clone()
        }

        #[test]
        fn a_failed_composition_reports_the_binary_that_produced_the_errors() {
            let data = data_of(CompositionError::Build {
                source: build_errors(),
                federation_version: FederationVersion::ExactFedTwo(Version::new(2, 9, 3)),
                provenance: Box::new(provenance()),
            });

            assert_that!(data).is_equal_to(json!({
                "plugins": [expected_entry()],
                "success": false,
            }));
        }

        #[test]
        fn a_failed_connector_run_reports_the_binary_it_ran() {
            let data = data_of(BinaryError::Exit {
                provenance: Box::new(provenance()),
                exit_code: Some(2),
                stdout: String::new(),
                stderr: "boom".to_string(),
            });

            assert_that!(data).is_equal_to(json!({
                "plugins": [expected_entry()],
                "success": false,
            }));
        }

        /// Empty, not absent. A plugin that never resolved has no source, level
        /// or path to report (FR57's one exclusion; FR87 carries its identity
        /// instead) — but the field still has to be there, or a consumer cannot
        /// tell "used no plugin" from "this Rover predates the field".
        #[rstest]
        #[case::resolution_never_reached_a_binary(CompositionError::InvalidSupergraphConfig(
            "bad config".to_string(),
        ))]
        #[case::nothing_resolved_before_the_write_failed(CompositionError::WriteFile {
            path: Utf8PathBuf::from("/tmp/supergraph.yaml"),
            error: Box::new(std::io::Error::other("disk full")),
            provenance: None,
        })]
        fn an_error_with_no_plugin_behind_it_reports_an_empty_array(
            #[case] error: CompositionError,
        ) {
            assert_that!(data_of(error)).is_equal_to(json!({ "plugins": [], "success": false }));
        }

        /// The commonest way a plugin-using command fails is never reaching a
        /// plugin at all — a missing or malformed `supergraph.yaml`. That is a
        /// different error type entirely, and the presence guarantee has to
        /// cover it or it doesn't cover the case anyone will actually hit.
        #[test]
        fn failing_before_resolution_still_reports_an_empty_array() {
            let error = CompositionPipelineError::Io(std::io::Error::other(
                "could not read supergraph.yaml",
            ));

            assert_that!(data_of(error)).is_equal_to(json!({ "plugins": [], "success": false }));
        }

        /// The run resolved a plugin, printed its FR54 line to stderr, and then
        /// failed for a reason that is not the plugin's fault. Reporting nothing
        /// here would have one run answering "which plugin?" two different ways
        /// on two channels.
        #[rstest]
        #[case::the_temporary_directory(CompositionError::TempDir {
            source: std::io::Error::other("no space left on device"),
            provenance: Some(Box::new(provenance())),
        })]
        #[case::the_config_write(CompositionError::WriteFile {
            path: Utf8PathBuf::from("/tmp/supergraph.yaml"),
            error: Box::new(std::io::Error::other("disk full")),
            provenance: Some(Box::new(provenance())),
        })]
        #[case::the_config_serialisation(CompositionError::SerdeYaml {
            source: serde_yaml::from_str::<serde_yaml::Value>("[1, 2").unwrap_err(),
            provenance: Some(Box::new(provenance())),
        })]
        fn a_failure_beside_the_plugin_still_names_it(#[case] error: CompositionError) {
            assert_that!(data_of(error)).is_equal_to(json!({
                "plugins": [expected_entry()],
                "success": false,
            }));
        }

        /// The Fixes entry this variant carries promises the cause is shown.
        /// It only is because `#[source]` puts it on the chain — the message
        /// itself deliberately does not interpolate it.
        #[test]
        fn the_temp_directory_failure_names_the_step_and_keeps_its_cause() {
            let error = RoverError::new(CompositionError::TempDir {
                source: std::io::Error::other("no space left on device"),
                provenance: None,
            });
            let value = serde_json::to_value(&error).unwrap();

            assert_that!(value["message"].as_str().unwrap())
                .is_equal_to("Failed to create a temporary directory for the composition input.");
            assert_that!(value["causes"]).is_equal_to(json!(["no space left on device"]));
        }

        #[test]
        fn every_error_the_binary_raises_carries_its_provenance() {
            let errors = [
                CompositionError::Binary {
                    error: "spawn failed".to_string(),
                    provenance: Box::new(provenance()),
                },
                CompositionError::BinaryExit {
                    exit_code: Some(2),
                    stdout: String::new(),
                    stderr: String::new(),
                    provenance: Box::new(provenance()),
                },
                CompositionError::InvalidOutput {
                    provenance: Box::new(provenance()),
                    error: "not json".to_string(),
                },
                CompositionError::EmptyOutput {
                    provenance: Box::new(provenance()),
                },
                CompositionError::InvalidInput {
                    provenance: Box::new(provenance()),
                    error: "bad version".to_string(),
                },
                CompositionError::Build {
                    source: build_errors(),
                    federation_version: FederationVersion::ExactFedTwo(Version::new(2, 9, 3)),
                    provenance: Box::new(provenance()),
                },
            ];

            let reported: Vec<Option<PluginProvenance>> = errors
                .iter()
                .map(|error| error.provenance().cloned())
                .collect();

            assert_that!(reported).is_equal_to(vec![Some(provenance()); 6]);
        }
    }
}
