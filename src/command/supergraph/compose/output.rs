use rover_std::Style;
use serde_json::{Value, json};

use crate::command::{
    CliOutput, install::PluginProvenance, supergraph::compose::CompositionOutput,
};

/// What `rover supergraph compose` prints: the composed schema on stdout, and
/// the hints the composer raised beside it on stderr.
#[derive(Debug)]
pub struct ComposeOutput {
    pub composition: CompositionOutput,
    /// The plugins this run resolved (FR57). Reported in JSON only; the plain
    /// output already names them on stderr as they are resolved.
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for ComposeOutput {
    fn text(&self) -> String {
        self.composition.supergraph_sdl.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        // `federation_version` is left out rather than null when unknown,
        // which is the shape this command has always emitted.
        Ok(match &self.composition.federation_version {
            Some(federation_version) => json!({
                "core_schema": self.composition.supergraph_sdl,
                "hints": self.composition.hints,
                "federation_version": federation_version,
                "plugins": self.plugins,
            }),
            None => json!({
                "core_schema": self.composition.supergraph_sdl,
                "hints": self.composition.hints,
                "plugins": self.plugins,
            }),
        })
    }

    fn descriptor(&self) -> Option<&str> {
        Some("Supergraph Schema")
    }

    /// The arm this replaced returned the SDL unconditionally, so an empty
    /// schema still printed its newline and `--output <file>` still wrote the
    /// file. Composition reaching success with no SDL is not a case anyone has
    /// seen, but "then the file stops being written" is not the way to find
    /// out, and the trait's default would do exactly that.
    fn prints_empty_output(&self) -> bool {
        true
    }

    /// Hints go beside the schema, never into it: whatever `text()` returns is
    /// the SDL a caller redirects into a file.
    fn stderr(&self) -> Option<String> {
        if self.composition.hints.is_empty() {
            return None;
        }

        let prefix = Style::HintPrefix.paint("HINT:");

        Some(
            self.composition
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
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::command::{
        RoverOutput,
        install::{PluginLevel, PluginSource},
    };

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
        output_with_plugins(federation_version, hints, Vec::new())
    }

    fn output_with_plugins(
        federation_version: Option<&str>,
        hints: Vec<BuildHint>,
        plugins: Vec<PluginProvenance>,
    ) -> ComposeOutput {
        ComposeOutput {
            composition: CompositionOutput {
                supergraph_sdl: String::from("type Query { hello: String }"),
                hints,
                federation_version: federation_version.map(String::from),
            },
            plugins,
        }
    }

    fn supergraph_plugin() -> PluginProvenance {
        PluginProvenance::new(
            "supergraph",
            Version::new(2, 9, 3),
            PluginSource::Downloaded,
            PluginLevel::Global,
            Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.9.3"),
        )
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
            "plugins": [],
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
            "plugins": [],
        }));
    }

    #[test]
    fn json_reports_the_plugins_the_run_resolved() {
        let actual = output_with_plugins(None, vec![], vec![supergraph_plugin()])
            .json()
            .unwrap();

        assert_that!(actual).is_equal_to(json!({
            "core_schema": "type Query { hello: String }",
            "hints": [],
            "plugins": [{
                "name": "supergraph",
                "version": "2.9.3",
                "source": "downloaded",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.9.3",
            }],
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

    /// Walks the real dispatcher rather than the type, because what's at stake
    /// is downstream of it: `write_or_print` skips the whole write-or-print
    /// block on `None`, so a `None` here is `--output <file>` silently not
    /// creating the file the removed arm always created.
    #[test]
    fn an_empty_schema_still_reaches_the_printer() {
        let empty = ComposeOutput {
            composition: CompositionOutput {
                supergraph_sdl: String::new(),
                hints: vec![],
                federation_version: None,
            },
            plugins: vec![],
        };

        let actual = RoverOutput::CliOutput(Box::new(empty))
            .get_stdout()
            .unwrap();

        assert_that!(actual).is_equal_to(Some(String::new()));
    }
}
