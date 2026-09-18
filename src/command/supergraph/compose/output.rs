use rover_std::Style;
use serde_json::{Value, json};

use crate::command::{CliOutput, supergraph::compose::CompositionOutput};

/// What `rover supergraph compose` prints: the composed schema on stdout, and
/// the hints the composer raised beside it on stderr.
#[derive(Debug)]
pub struct ComposeOutput(pub CompositionOutput);

impl CliOutput for ComposeOutput {
    fn text(&self) -> String {
        self.0.supergraph_sdl.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        // `federation_version` is left out rather than null when unknown,
        // which is the shape this command has always emitted.
        Ok(match &self.0.federation_version {
            Some(federation_version) => json!({
                "core_schema": self.0.supergraph_sdl,
                "hints": self.0.hints,
                "federation_version": federation_version,
            }),
            None => json!({
                "core_schema": self.0.supergraph_sdl,
                "hints": self.0.hints,
            }),
        })
    }

    fn descriptor(&self) -> Option<&str> {
        Some("Supergraph Schema")
    }

    /// Hints go beside the schema, never into it: whatever `text()` returns is
    /// the SDL a caller redirects into a file.
    fn stderr(&self) -> Option<String> {
        if self.0.hints.is_empty() {
            return None;
        }

        let prefix = Style::HintPrefix.paint("HINT:");

        Some(
            self.0
                .hints
                .iter()
                .map(|hint| format!("{} {}", prefix, hint.message))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::rover::BuildHint;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn hint(message: &str) -> BuildHint {
        BuildHint {
            message: message.to_string(),
            code: None,
            nodes: None,
            omitted_nodes_count: None,
            other: Default::default(),
        }
    }

    fn output(federation_version: Option<&str>, hints: Vec<BuildHint>) -> ComposeOutput {
        ComposeOutput(CompositionOutput {
            supergraph_sdl: String::from("type Query { hello: String }"),
            hints,
            federation_version: federation_version.map(String::from),
        })
    }

    #[test]
    fn json_carries_the_federation_version_when_known() {
        let actual = output(Some("2.9.3"), vec![hint("a hint")]).json().unwrap();

        assert_that!(actual).is_equal_to(json!({
            "core_schema": "type Query { hello: String }",
            "hints": [{
                "message": "a hint",
                "code": null,
                "nodes": null,
                "omittedNodesCount": null,
            }],
            "federation_version": "2.9.3",
        }));
    }

    /// Absent, not null: scripts reading this field have always been able to
    /// treat its presence as the question.
    #[test]
    fn json_omits_the_federation_version_when_unknown() {
        let actual = output(None, vec![]).json().unwrap();

        assert_that!(actual).is_equal_to(json!({
            "core_schema": "type Query { hello: String }",
            "hints": [],
        }));
    }

    #[test]
    fn text_is_the_schema_alone() {
        let actual = output(Some("2.9.3"), vec![hint("a hint")]).text();

        assert_that!(actual).is_equal_to(String::from("type Query { hello: String }"));
    }

    #[test]
    fn descriptor_names_the_schema() {
        assert_that!(output(None, vec![]).descriptor()).is_equal_to(Some("Supergraph Schema"));
    }

    #[rstest]
    #[case::one(vec![hint("first")], "first")]
    #[case::several(vec![hint("first"), hint("second")], "first\nsecond")]
    fn stderr_lists_every_hint(#[case] hints: Vec<BuildHint>, #[case] expected: &str) {
        let actual = output(None, hints).stderr().unwrap();

        // The prefix is styled, so compare against the same paint call rather
        // than a literal that would differ under a colour-capable terminal.
        let prefix = Style::HintPrefix.paint("HINT:");
        let expected = expected
            .lines()
            .map(|message| format!("{prefix} {message}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_that!(actual).is_equal_to(expected);
    }

    /// No hints means no stderr at all — the legacy path printed a stray blank
    /// line here.
    #[test]
    fn stderr_is_silent_without_hints() {
        assert_that!(output(None, vec![]).stderr()).is_equal_to(None);
    }
}
