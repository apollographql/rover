use pluralizer::pluralize;
use rover_client::{
    operations::subgraph::publish::SubgraphPublishResponse,
    shared::{DownstreamLaunch, LaunchStatus},
};
use rover_std::{Style, hyperlink};

use crate::command::CliOutput;

/// [`CliOutput`] for the launch/downstream-launch portion of `rover subgraph
/// publish`'s response. Used inline by `Publish::run` -- like
/// `crate::command::graph::publish::output::GraphPublishLaunchesOutput` --
/// rather than returned as the command's own `RoverOutput`, so the existing
/// stdout contract for a successful publish is unaffected. `exit_code()`
/// decides whether a triggered launch failure should fail the command.
#[derive(Debug)]
pub(super) struct SubgraphPublishLaunchesOutput<'a>(pub &'a SubgraphPublishResponse);

impl SubgraphPublishLaunchesOutput<'_> {
    fn has_blocking_failure(&self) -> bool {
        self.0.launch_status == Some(LaunchStatus::FAILED)
            || self.failed_downstream_launches().next().is_some()
    }

    fn failed_downstream_launches(&self) -> impl Iterator<Item = &DownstreamLaunch> {
        self.0
            .downstream_launches
            .iter()
            .filter(|launch| launch.status == LaunchStatus::FAILED)
    }
}

impl CliOutput for SubgraphPublishLaunchesOutput<'_> {
    fn exit_code(&self) -> i32 {
        if self.has_blocking_failure() { 1 } else { 0 }
    }

    fn text(&self) -> String {
        if self.has_blocking_failure() {
            let failed_variants = self
                .failed_downstream_launches()
                .map(|launch| launch.variant_name.as_str())
                .collect::<Vec<_>>();
            let detail = if failed_variants.is_empty() {
                "the launch itself failed".to_string()
            } else {
                format!(
                    "{} failed: {}",
                    pluralize(
                        "downstream contract launch",
                        failed_variants.len() as isize,
                        false
                    ),
                    Style::Variant.paint(failed_variants.join(", "))
                )
            };
            // Prefer the source launch's own URL; `launchUrl` (from the
            // mutation) and the polled launch status are independently
            // nullable, so a failed launch can exist with no source URL --
            // fall back to a failed downstream launch's URL, which is
            // always hand-built and populated.
            let url = self.0.launch_url.as_deref().or_else(|| {
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
        } else if let Some(launch_url) = &self.0.launch_url
            && !self.0.downstream_launches.is_empty()
        {
            let variants = self
                .0
                .downstream_launches
                .iter()
                .map(|launch| launch.variant_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Triggered downstream launches for {}: {}.\nView launch details at: {}",
                pluralize(
                    "contract variant",
                    self.0.downstream_launches.len() as isize,
                    true
                ),
                Style::Variant.paint(variants),
                hyperlink(launch_url)
            )
        } else {
            String::new()
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(serde_json::json!({
            "launch_url": self.0.launch_url,
            "launch_status": self.0.launch_status,
            "launch_superseded": self.0.launch_superseded,
            "downstream_launches": self.0.downstream_launches,
        }))
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::rover::BuildErrors;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn response(
        launch_url: Option<&str>,
        launch_status: Option<LaunchStatus>,
        downstream_launches: Vec<DownstreamLaunch>,
    ) -> SubgraphPublishResponse {
        response_with_superseded(launch_url, launch_status, false, downstream_launches)
    }

    fn response_with_superseded(
        launch_url: Option<&str>,
        launch_status: Option<LaunchStatus>,
        launch_superseded: bool,
        downstream_launches: Vec<DownstreamLaunch>,
    ) -> SubgraphPublishResponse {
        SubgraphPublishResponse {
            api_schema_hash: Some("123456".to_string()),
            supergraph_was_updated: true,
            subgraph_was_created: false,
            subgraph_was_updated: true,
            build_errors: BuildErrors::new(),
            launch_url: launch_url.map(str::to_string),
            launch_cli_copy: None,
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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        assert_that!(SubgraphPublishLaunchesOutput(&response).exit_code()).is_equal_to(expected);
    }

    #[test]
    fn json_serializes_the_launch_fields() {
        let response = response(
            Some("https://studio.apollographql.com/graph/my-graph/launches/launch-1"),
            Some(LaunchStatus::COMPLETED),
            vec![downstream("mobile", LaunchStatus::COMPLETED)],
        );
        let json = SubgraphPublishLaunchesOutput(&response).json().unwrap();

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
        let output = SubgraphPublishLaunchesOutput(&response);

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
        let output = SubgraphPublishLaunchesOutput(&response);

        assert_that!(output.exit_code()).is_equal_to(0);
        let json = output.json().unwrap();
        assert_that!(json["downstream_launches"][0]["superseded"])
            .is_equal_to(serde_json::json!(true));
    }
}
