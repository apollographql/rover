use pluralizer::pluralize;
use rover_client::{
    operations::{
        graph::publish::GraphPublishResponse, subgraph::publish::SubgraphPublishResponse,
    },
    shared::{DownstreamLaunch, LaunchStatus},
};
use rover_std::{Style, hyperlink};

use crate::command::CliOutput;

/// Implemented by a `graph publish`/`subgraph publish` response that reports
/// a triggered launch (and any downstream contract-variant launches it
/// triggered). Lets `PublishLaunchesOutput`'s rendering be shared between the
/// two commands instead of duplicated, without requiring their
/// otherwise-different response types to share a field layout.
///
/// Deliberately local to this (CLI/presentation) crate rather than living on
/// `rover-client`'s shared types, even though it's implemented on
/// `rover-client` types (`GraphPublishResponse`, `SubgraphPublishResponse`)
/// -- Rust's orphan rules allow a local trait on a foreign type, and this is
/// presentation logic, not client/data logic (see
/// `crate::command::check_output`'s doc comment for the same principle
/// applied to `CheckWorkflowOutput`).
pub(crate) trait PublishLaunches {
    fn launch_url(&self) -> Option<&str>;
    fn launch_status(&self) -> Option<LaunchStatus>;
    fn launch_superseded(&self) -> bool;
    fn downstream_launches(&self) -> &[DownstreamLaunch];
}

impl PublishLaunches for GraphPublishResponse {
    fn launch_url(&self) -> Option<&str> {
        self.launch_url.as_deref()
    }

    fn launch_status(&self) -> Option<LaunchStatus> {
        self.launch_status.clone()
    }

    fn launch_superseded(&self) -> bool {
        self.launch_superseded
    }

    fn downstream_launches(&self) -> &[DownstreamLaunch] {
        &self.downstream_launches
    }
}

impl PublishLaunches for SubgraphPublishResponse {
    fn launch_url(&self) -> Option<&str> {
        self.launch_url.as_deref()
    }

    fn launch_status(&self) -> Option<LaunchStatus> {
        self.launch_status.clone()
    }

    fn launch_superseded(&self) -> bool {
        self.launch_superseded
    }

    fn downstream_launches(&self) -> &[DownstreamLaunch] {
        &self.downstream_launches
    }
}

/// [`CliOutput`] for the launch/downstream-launch portion of a `graph
/// publish`/`subgraph publish` response -- shared by both commands, like
/// `crate::command::check_output::CheckWorkflowOutput` is for their `--check`
/// flow. Used inline by each command's `Publish::run` rather than returned as
/// the command's own `RoverOutput`, so each command's existing stdout
/// contract is unaffected. `exit_code()` decides whether a triggered launch
/// failure should fail the command.
#[derive(Debug)]
pub(crate) struct PublishLaunchesOutput<'a, T>(pub &'a T);

impl<T: PublishLaunches> PublishLaunchesOutput<'_, T> {
    fn has_blocking_failure(&self) -> bool {
        self.0.launch_status() == Some(LaunchStatus::FAILED)
            || self.failed_downstream_launches().next().is_some()
    }

    fn failed_downstream_launches(&self) -> impl Iterator<Item = &DownstreamLaunch> {
        self.0
            .downstream_launches()
            .iter()
            .filter(|launch| launch.status == LaunchStatus::FAILED)
    }

    /// Whether `text()` renders anything, without callers outside this
    /// module having to re-derive the rule. Used by
    /// `RoverOutput::SubgraphPublishResponse`'s text rendering to decide
    /// whether `launch_cli_copy` (Studio-authored copy that also mentions
    /// the launch URL) would repeat a link this report already printed.
    pub(crate) fn reports_launches(&self) -> bool {
        self.has_blocking_failure() || !self.0.downstream_launches().is_empty()
    }
}

impl<T: PublishLaunches + std::fmt::Debug + Sync> CliOutput for PublishLaunchesOutput<'_, T> {
    fn exit_code(&self) -> i32 {
        if self.has_blocking_failure() { 1 } else { 0 }
    }

    fn text(&self) -> String {
        if self.has_blocking_failure() {
            let source_failed = self.0.launch_status() == Some(LaunchStatus::FAILED);
            let failed_variants = self
                .failed_downstream_launches()
                .map(|launch| launch.variant_name.as_str())
                .collect::<Vec<_>>();
            // Both can be true at once (the source launch itself failed *and*
            // a downstream launch failed) -- report whichever apply, instead
            // of letting one silently eclipse the other.
            let mut details = Vec::new();
            if source_failed {
                details.push("the launch itself failed".to_string());
            }
            if !failed_variants.is_empty() {
                details.push(format!(
                    "{} failed: {}",
                    pluralize(
                        "downstream contract launch",
                        failed_variants.len() as isize,
                        false
                    ),
                    Style::Variant.paint(failed_variants.join(", "))
                ));
            }
            let detail = details.join(", and ");
            // Prefer the source launch's own URL; the mutation's `launchUrl`
            // and the polled launch status are independently nullable, so a
            // failed launch can exist with no source URL -- fall back to a
            // failed downstream launch's URL, which is always hand-built and
            // populated.
            let url = self.0.launch_url().or_else(|| {
                self.failed_downstream_launches()
                    .next()
                    .map(|launch| launch.url.as_str())
            });
            match url {
                Some(url) => format!(
                    "The publish succeeded, but {detail}.\nView launch details at: {}",
                    hyperlink(url)
                ),
                None => format!("The publish succeeded, but {detail}."),
            }
        } else if !self.0.downstream_launches().is_empty() {
            // Every `DownstreamLaunch` always has a populated `url`, so
            // falling back to the first one when the source launch's own
            // `launchUrl` is null (independently nullable from this success
            // path existing at all) always yields a real link.
            let url = self
                .0
                .launch_url()
                .unwrap_or_else(|| self.0.downstream_launches()[0].url.as_str());
            let variants = self
                .0
                .downstream_launches()
                .iter()
                .map(|launch| launch.variant_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Triggered downstream launches for {}: {}.\nView launch details at: {}",
                pluralize(
                    "contract variant",
                    self.0.downstream_launches().len() as isize,
                    true
                ),
                Style::Variant.paint(variants),
                hyperlink(url)
            )
        } else {
            String::new()
        }
    }

    /// Not wired into production `--format json` output -- `RoverOutput`'s
    /// `GraphPublishResponse`/`SubgraphPublishResponse` JSON handling
    /// serializes the whole response directly (`json!(publish_response)`),
    /// which already includes these same four fields alongside the rest of
    /// the response. Implemented anyway since `CliOutput::json` has no
    /// default, and exercised by this module's own tests; kept hand-rolled
    /// rather than re-deriving from `T`'s own `Serialize` output because `T`
    /// isn't required to implement `Serialize` here, only `PublishLaunches`.
    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(serde_json::json!({
            "launch_url": self.0.launch_url(),
            "launch_status": self.0.launch_status(),
            "launch_superseded": self.0.launch_superseded(),
            "downstream_launches": self.0.downstream_launches(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    /// A minimal stand-in for `GraphPublishResponse`/`SubgraphPublishResponse`,
    /// used so these tests exercise the shared rendering logic once, in
    /// isolation, rather than being duplicated per response type.
    #[derive(Debug)]
    struct TestLaunches {
        launch_url: Option<String>,
        launch_status: Option<LaunchStatus>,
        launch_superseded: bool,
        downstream_launches: Vec<DownstreamLaunch>,
    }

    impl PublishLaunches for TestLaunches {
        fn launch_url(&self) -> Option<&str> {
            self.launch_url.as_deref()
        }

        fn launch_status(&self) -> Option<LaunchStatus> {
            self.launch_status.clone()
        }

        fn launch_superseded(&self) -> bool {
            self.launch_superseded
        }

        fn downstream_launches(&self) -> &[DownstreamLaunch] {
            &self.downstream_launches
        }
    }

    fn response(
        launch_url: Option<&str>,
        launch_status: Option<LaunchStatus>,
        downstream_launches: Vec<DownstreamLaunch>,
    ) -> TestLaunches {
        response_with_superseded(launch_url, launch_status, false, downstream_launches)
    }

    fn response_with_superseded(
        launch_url: Option<&str>,
        launch_status: Option<LaunchStatus>,
        launch_superseded: bool,
        downstream_launches: Vec<DownstreamLaunch>,
    ) -> TestLaunches {
        TestLaunches {
            launch_url: launch_url.map(str::to_string),
            launch_status,
            launch_superseded,
            downstream_launches,
        }
    }

    fn downstream(variant_name: &str, status: LaunchStatus) -> DownstreamLaunch {
        downstream_with_superseded(variant_name, status, false)
    }

    fn downstream_with_superseded(
        variant_name: &str,
        status: LaunchStatus,
        superseded: bool,
    ) -> DownstreamLaunch {
        DownstreamLaunch {
            graph_id: "my-graph".to_string(),
            variant_name: variant_name.to_string(),
            status,
            superseded,
            url: format!("https://studio.apollographql.com/graph/my-graph/launches/{variant_name}"),
        }
    }

    #[test]
    fn no_launch_reports_nothing() {
        let response = response(None, None, Vec::new());
        let output = PublishLaunchesOutput(&response);

        assert_that!(output.text()).is_equal_to(String::new());
        assert_that!(output.exit_code()).is_equal_to(0);
    }

    #[test]
    fn launch_with_no_downstream_launches_reports_nothing() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            Vec::new(),
        );
        let output = PublishLaunchesOutput(&response);

        assert_that!(output.text()).is_equal_to(String::new());
        assert_that!(output.exit_code()).is_equal_to(0);
    }

    #[test]
    fn successful_downstream_launches_are_reported_with_zero_exit_code() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![
                downstream("mobile", LaunchStatus::COMPLETED),
                downstream("partner-api", LaunchStatus::COMPLETED),
            ],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "Triggered downstream launches for 2 contract variants: mobile, partner-api.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(0);
    }

    #[test]
    fn a_failed_source_launch_with_no_downstream_launches_fails_with_nonzero_exit_code() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::FAILED),
            Vec::new(),
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "The publish succeeded, but the launch itself failed.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(1);
    }

    #[test]
    fn a_failed_downstream_launch_fails_with_nonzero_exit_code() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![
                downstream("mobile", LaunchStatus::COMPLETED),
                downstream("partner-api", LaunchStatus::FAILED),
            ],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "The publish succeeded, but downstream contract launch failed: partner-api.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(1);
    }

    #[test]
    fn multiple_failed_downstream_launches_pluralize_correctly() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![
                downstream("mobile", LaunchStatus::FAILED),
                downstream("partner-api", LaunchStatus::FAILED),
            ],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "The publish succeeded, but downstream contract launches failed: mobile, partner-api.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
        );
    }

    /// `launch_url` (from the mutation) and `launch_status` (from polling)
    /// are independently nullable -- a failed launch with no source URL is
    /// reachable, and must still render a failure message (and still exit
    /// non-zero) rather than the empty string `exit_code() != 0` would then
    /// have nothing to point at.
    #[test]
    fn a_failed_source_launch_with_no_launch_url_still_reports_and_fails() {
        let response = response(None, Some(LaunchStatus::FAILED), Vec::new());
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text)
            .is_equal_to("The publish succeeded, but the launch itself failed.".to_string());
        assert_that!(output.exit_code()).is_equal_to(1);
    }

    /// Same as above, but for a failed downstream launch -- falls back to
    /// that launch's own (always-populated) URL instead of omitting the
    /// link line entirely.
    #[test]
    fn a_failed_downstream_launch_with_no_source_launch_url_falls_back_to_its_own_url() {
        let response = response(
            None,
            Some(LaunchStatus::COMPLETED),
            vec![downstream("partner-api", LaunchStatus::FAILED)],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "The publish succeeded, but downstream contract launch failed: partner-api.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/partner-api".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(1);
    }

    #[rstest]
    #[case::completed(LaunchStatus::COMPLETED, 0)]
    #[case::failed(LaunchStatus::FAILED, 1)]
    #[case::initiated(LaunchStatus::INITIATED, 0)]
    fn exit_code_matches_source_launch_status(#[case] status: LaunchStatus, #[case] expected: i32) {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(status),
            Vec::new(),
        );
        assert_that!(PublishLaunchesOutput(&response).exit_code()).is_equal_to(expected);
    }

    #[rstest]
    #[case::no_launch(None, Vec::new(), false)]
    #[case::launch_with_no_downstream_launches(Some(LaunchStatus::COMPLETED), Vec::new(), false)]
    #[case::successful_downstream_launches(
        Some(LaunchStatus::COMPLETED),
        vec![downstream("mobile", LaunchStatus::COMPLETED)],
        true
    )]
    #[case::failed_source_launch_with_no_downstream_launches(
        Some(LaunchStatus::FAILED),
        Vec::new(),
        true
    )]
    fn reports_launches_matches_whether_text_is_empty(
        #[case] launch_status: Option<LaunchStatus>,
        #[case] downstream_launches: Vec<DownstreamLaunch>,
        #[case] expected: bool,
    ) {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            launch_status,
            downstream_launches,
        );
        let output = PublishLaunchesOutput(&response);

        assert_that!(output.reports_launches()).is_equal_to(expected);
        assert_that!(output.text().is_empty()).is_equal_to(!expected);
    }

    #[test]
    fn json_serializes_the_launch_fields() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![downstream("mobile", LaunchStatus::COMPLETED)],
        );
        let json = PublishLaunchesOutput(&response).json().unwrap();

        assert_that!(json).is_equal_to(serde_json::json!({
            "launch_url": "https://studio.apollographql.com/graph/my-graph/launches/launch-1",
            "launch_status": "COMPLETED",
            "launch_superseded": false,
            "downstream_launches": [
                {
                    "graph_id": "my-graph",
                    "variant_name": "mobile",
                    "status": "COMPLETED",
                    "superseded": false,
                    "url": "https://studio.apollographql.com/graph/my-graph/launches/mobile"
                }
            ]
        }));
    }

    /// A superseded source launch keeps `status == INITIATED` (per the API),
    /// but must not be treated as a blocking failure, and `superseded` shows
    /// up in the JSON data alongside it.
    #[test]
    fn a_superseded_source_launch_is_not_a_blocking_failure_and_appears_in_json() {
        let response = response_with_superseded(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::INITIATED),
            true,
            Vec::new(),
        );
        let output = PublishLaunchesOutput(&response);

        assert_that!(output.exit_code()).is_equal_to(0);
        assert_that!(output.json().unwrap()).is_equal_to(serde_json::json!({
            "launch_url": "https://studio.apollographql.com/graph/my-graph/launches/launch-1",
            "launch_status": "INITIATED",
            "launch_superseded": true,
            "downstream_launches": []
        }));
    }

    /// Same as above, but for a downstream contract-variant launch.
    #[test]
    fn a_superseded_downstream_launch_is_not_a_blocking_failure_and_appears_in_json() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![downstream_with_superseded(
                "mobile",
                LaunchStatus::INITIATED,
                true,
            )],
        );
        let output = PublishLaunchesOutput(&response);

        assert_that!(output.exit_code()).is_equal_to(0);
        assert_that!(output.json().unwrap()).is_equal_to(serde_json::json!({
            "launch_url": "https://studio.apollographql.com/graph/my-graph/launches/launch-1",
            "launch_status": "COMPLETED",
            "launch_superseded": false,
            "downstream_launches": [
                {
                    "graph_id": "my-graph",
                    "variant_name": "mobile",
                    "status": "INITIATED",
                    "superseded": true,
                    "url": "https://studio.apollographql.com/graph/my-graph/launches/mobile"
                }
            ]
        }));
    }

    /// A publish with completed downstream launches but no source `launchUrl`
    /// (independently nullable from the polled launch status) must still
    /// render a report, falling back to the first downstream launch's own
    /// (always-populated) URL -- same fix as the failure-branch fallback
    /// above, applied to the success branch.
    #[test]
    fn successful_downstream_launches_with_no_source_launch_url_falls_back_to_its_own_url() {
        let response = response(
            None,
            Some(LaunchStatus::COMPLETED),
            vec![downstream("mobile", LaunchStatus::COMPLETED)],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "Triggered downstream launches for 1 contract variant: mobile.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/mobile".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(0);
    }

    /// When the source launch *and* a downstream launch both fail, the
    /// message must mention both -- not let one silently eclipse the other.
    #[test]
    fn a_failed_source_launch_and_a_failed_downstream_launch_are_both_reported() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::FAILED),
            vec![downstream("partner-api", LaunchStatus::FAILED)],
        );
        let output = PublishLaunchesOutput(&response);

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());
        assert_that!(text).is_equal_to(
            "The publish succeeded, but the launch itself failed, and downstream contract launch failed: partner-api.\nView launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
        );
        assert_that!(output.exit_code()).is_equal_to(1);
    }

    /// Every unit test above goes through the local `TestLaunches` stand-in,
    /// so nothing binds `PublishLaunches`'s real `GraphPublishResponse` impl
    /// to this rendering -- a transposed field there would go unnoticed.
    /// Exercise the accessors directly against a real response.
    #[test]
    fn graph_publish_response_implements_publish_launches_correctly() {
        use rover_client::operations::graph::publish::{ChangeSummary, FieldChanges, TypeChanges};

        let response = GraphPublishResponse {
            api_schema_hash: "123456".to_string(),
            change_summary: ChangeSummary {
                field_changes: FieldChanges {
                    additions: 0,
                    removals: 0,
                    edits: 0,
                },
                type_changes: TypeChanges {
                    additions: 0,
                    removals: 0,
                    edits: 0,
                },
            },
            total_type_count: 5,
            launch_url: Some(
                "https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
            ),
            launch_status: Some(LaunchStatus::FAILED),
            launch_superseded: true,
            downstream_launches: vec![downstream("mobile", LaunchStatus::COMPLETED)],
        };

        assert_that!(PublishLaunches::launch_url(&response)).is_equal_to(Some(
            "https://studio.apollographql.com/graph/my-graph/launches/launch-1",
        ));
        assert_that!(PublishLaunches::launch_status(&response))
            .is_equal_to(Some(LaunchStatus::FAILED));
        assert_that!(PublishLaunches::launch_superseded(&response)).is_true();
        assert_that!(PublishLaunches::downstream_launches(&response))
            .is_equal_to(&[downstream("mobile", LaunchStatus::COMPLETED)][..]);
    }

    /// Same rationale as `graph_publish_response_implements_publish_launches_correctly`,
    /// for `SubgraphPublishResponse`'s impl.
    #[test]
    fn subgraph_publish_response_implements_publish_launches_correctly() {
        use apollo_federation_types::rover::BuildErrors;

        let response = SubgraphPublishResponse {
            api_schema_hash: Some("123456".to_string()),
            supergraph_was_updated: true,
            subgraph_was_created: false,
            subgraph_was_updated: true,
            build_errors: BuildErrors::new(),
            launch_url: Some(
                "https://studio.apollographql.com/graph/my-graph/launches/launch-1".to_string(),
            ),
            launch_cli_copy: None,
            launch_status: Some(LaunchStatus::FAILED),
            launch_superseded: true,
            downstream_launches: vec![downstream("mobile", LaunchStatus::COMPLETED)],
        };

        assert_that!(PublishLaunches::launch_url(&response)).is_equal_to(Some(
            "https://studio.apollographql.com/graph/my-graph/launches/launch-1",
        ));
        assert_that!(PublishLaunches::launch_status(&response))
            .is_equal_to(Some(LaunchStatus::FAILED));
        assert_that!(PublishLaunches::launch_superseded(&response)).is_true();
        assert_that!(PublishLaunches::downstream_launches(&response))
            .is_equal_to(&[downstream("mobile", LaunchStatus::COMPLETED)][..]);
    }
}
