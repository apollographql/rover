use std::fmt;

use rover_studio::types::GraphRef;
use rover_tower::poll_retry::{PollOutcome, SimplePollOutcome};
use serde::Serialize;

use crate::{
    operations::graph::publish::{
        graph_publish_launch_status_query, runner::graph_publish_mutation,
    },
    shared::{GitContext, LaunchStatus},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPublishInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub changelog_message: Option<String>,
}

type MutationVariables = graph_publish_mutation::Variables;
impl From<GraphPublishInput> for MutationVariables {
    fn from(input: GraphPublishInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            proposed_schema: input.proposed_schema,
            git_context: graph_publish_mutation::GitContextInput {
                branch: input.git_context.branch,
                commit: input.git_context.commit,
                committer: input.git_context.author,
                remote_url: input.git_context.remote_url,
                message: input.changelog_message,
            },
        }
    }
}

/// Input to fetch the status of a launch (and its downstream contract-variant
/// launches) triggered by a `graph publish`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaunchStatusInput {
    pub graph_ref: GraphRef,
    pub launch_id: String,
}

impl From<LaunchStatusInput> for graph_publish_launch_status_query::Variables {
    fn from(input: LaunchStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            launch_id: input.launch_id,
        }
    }
}

/// A snapshot of a downstream contract-variant launch's status, used to drive
/// polling. Distinct from the shared `DownstreamLaunch` report type, which is
/// only built once every launch reaches a terminal status and needs a URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DownstreamLaunchSnapshot {
    pub launch_id: String,
    pub graph_id: String,
    pub variant_name: String,
    pub status: LaunchStatus,
    /// A superseded launch's `status` stays `INITIATED` forever per the API;
    /// this is the only way to tell it apart from one still in flight.
    pub superseded: bool,
}

/// A snapshot of a launch's (and its downstream contract-variant launches')
/// status, used to drive polling until every launch reaches a terminal state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaunchSnapshot {
    pub launch_id: String,
    pub graph_id: String,
    pub status: LaunchStatus,
    pub superseded: bool,
    pub downstream_launches: Vec<DownstreamLaunchSnapshot>,
}

impl PollOutcome for LaunchSnapshot {
    fn poll_outcome(&self) -> SimplePollOutcome {
        // A superseded launch's status remains INITIATED forever, so it must
        // be treated as terminal even though `is_pending` would otherwise say
        // it's still running -- see `Launch.status`'s doc comment in the
        // schema.
        let is_pending = |status: &LaunchStatus, superseded: bool| {
            *status == LaunchStatus::INITIATED && !superseded
        };
        if is_pending(&self.status, self.superseded)
            || self
                .downstream_launches
                .iter()
                .any(|launch| is_pending(&launch.status, launch.superseded))
        {
            SimplePollOutcome::Incomplete
        } else {
            SimplePollOutcome::Complete
        }
    }
}

type QueryLaunchStatus = graph_publish_launch_status_query::LaunchStatus;
impl From<QueryLaunchStatus> for LaunchStatus {
    fn from(status: QueryLaunchStatus) -> Self {
        match status {
            QueryLaunchStatus::LAUNCH_COMPLETED => LaunchStatus::COMPLETED,
            QueryLaunchStatus::LAUNCH_FAILED => LaunchStatus::FAILED,
            QueryLaunchStatus::LAUNCH_INITIATED => LaunchStatus::INITIATED,
            // A status this client's schema snapshot doesn't recognize. Treated as
            // terminal-and-failed rather than pending, so an unrecognized future
            // status can't cause the poll loop to spin until it times out.
            QueryLaunchStatus::Other(_) => LaunchStatus::FAILED,
        }
    }
}

type QueryLaunch =
    graph_publish_launch_status_query::GraphPublishLaunchStatusQueryGraphVariantLaunch;
impl From<QueryLaunch> for LaunchSnapshot {
    fn from(launch: QueryLaunch) -> Self {
        LaunchSnapshot {
            launch_id: launch.id,
            graph_id: launch.graph_id,
            status: launch.status.into(),
            superseded: launch.superseded_at.is_some(),
            downstream_launches: launch
                .downstream_launches
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

type QueryDownstreamLaunch = graph_publish_launch_status_query::GraphPublishLaunchStatusQueryGraphVariantLaunchDownstreamLaunches;
impl From<QueryDownstreamLaunch> for DownstreamLaunchSnapshot {
    fn from(launch: QueryDownstreamLaunch) -> Self {
        DownstreamLaunchSnapshot {
            launch_id: launch.id,
            graph_id: launch.graph_id,
            variant_name: launch.graph_variant,
            status: launch.status.into(),
            superseded: launch.superseded_at.is_some(),
        }
    }
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct GraphPublishResponse {
    pub api_schema_hash: String,
    #[serde(flatten)]
    pub change_summary: ChangeSummary,
    pub total_type_count: u64,
    /// A link to the publish's own launch. `None` when the publish triggered
    /// no launch at all.
    pub launch_url: Option<String>,
    /// The publish's own launch's terminal status. `None` when the publish
    /// triggered no launch at all. Carries the outcome as data rather than
    /// as an error, matching contract/subgraph preview's
    /// success-or-failure-is-still-data approach for an async follow-up.
    pub launch_status: Option<LaunchStatus>,
    /// Whether the publish's own launch was superseded by a later one (e.g. a
    /// concurrent publish to the same variant). Always `false` when
    /// `launch_status` is `None`.
    pub launch_superseded: bool,
    pub downstream_launches: Vec<crate::shared::DownstreamLaunch>,
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct ChangeSummary {
    pub field_changes: FieldChanges,
    pub type_changes: TypeChanges,
}

impl ChangeSummary {
    pub(crate) const fn none() -> ChangeSummary {
        ChangeSummary {
            field_changes: FieldChanges::none(),
            type_changes: TypeChanges::none(),
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.field_changes.is_none() && self.type_changes.is_none()
    }
}

impl fmt::Display for ChangeSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_none() {
            write!(f, "[No Changes]")
        } else {
            write!(f, "[{}, {}]", self.field_changes, self.type_changes)
        }
    }
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct FieldChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl FieldChanges {
    pub(crate) const fn none() -> FieldChanges {
        FieldChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }

    pub(crate) const fn with_diff(additions: u64, removals: u64, edits: u64) -> FieldChanges {
        FieldChanges {
            additions,
            removals,
            edits,
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.additions == 0 && self.removals == 0 && self.edits == 0
    }
}

impl fmt::Display for FieldChanges {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Fields: +{} -{} △ {}",
            self.additions, self.removals, self.edits
        )
    }
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct TypeChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl TypeChanges {
    pub(crate) const fn none() -> TypeChanges {
        TypeChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }

    pub(crate) const fn with_diff(additions: u64, removals: u64, edits: u64) -> TypeChanges {
        TypeChanges {
            additions,
            removals,
            edits,
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.additions == 0 && self.removals == 0 && self.edits == 0
    }
}

impl fmt::Display for TypeChanges {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Types: +{} -{} △ {}",
            self.additions, self.removals, self.edits
        )
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn snapshot(status: LaunchStatus, downstream: Vec<LaunchStatus>) -> LaunchSnapshot {
        LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "my-graph".to_string(),
            status,
            superseded: false,
            downstream_launches: downstream
                .into_iter()
                .enumerate()
                .map(|(i, status)| DownstreamLaunchSnapshot {
                    launch_id: format!("launch-{i}"),
                    graph_id: "my-graph".to_string(),
                    variant_name: format!("variant-{i}"),
                    status,
                    superseded: false,
                })
                .collect(),
        }
    }

    #[rstest]
    #[case::source_pending(LaunchStatus::INITIATED, vec![], SimplePollOutcome::Incomplete)]
    #[case::source_completed_no_downstream(LaunchStatus::COMPLETED, vec![], SimplePollOutcome::Complete)]
    #[case::source_completed_downstream_pending(
        LaunchStatus::COMPLETED,
        vec![LaunchStatus::INITIATED],
        SimplePollOutcome::Incomplete
    )]
    #[case::source_completed_downstream_completed(
        LaunchStatus::COMPLETED,
        vec![LaunchStatus::COMPLETED, LaunchStatus::COMPLETED],
        SimplePollOutcome::Complete
    )]
    #[case::source_completed_one_downstream_failed(
        LaunchStatus::COMPLETED,
        vec![LaunchStatus::COMPLETED, LaunchStatus::FAILED],
        SimplePollOutcome::Complete
    )]
    fn poll_outcome_is_incomplete_only_while_something_is_still_initiated(
        #[case] status: LaunchStatus,
        #[case] downstream: Vec<LaunchStatus>,
        #[case] expected: SimplePollOutcome,
    ) {
        assert_that!(snapshot(status, downstream).poll_outcome()).is_equal_to(expected);
    }

    /// A superseded launch's `status` stays `INITIATED` forever per the API
    /// (see `Launch.status`'s schema doc comment) -- `poll_outcome` must
    /// treat it as terminal anyway, or the poll never ends.
    #[test]
    fn poll_outcome_is_complete_when_the_source_launch_is_superseded_while_initiated() {
        let snapshot = LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "my-graph".to_string(),
            status: LaunchStatus::INITIATED,
            superseded: true,
            downstream_launches: Vec::new(),
        };

        assert_that!(snapshot.poll_outcome()).is_equal_to(SimplePollOutcome::Complete);
    }

    /// Same as above, but for a downstream contract-variant launch rather
    /// than the source launch.
    #[test]
    fn poll_outcome_is_complete_when_a_downstream_launch_is_superseded_while_initiated() {
        let snapshot = LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "my-graph".to_string(),
            status: LaunchStatus::COMPLETED,
            superseded: false,
            downstream_launches: vec![DownstreamLaunchSnapshot {
                launch_id: "launch-2".to_string(),
                graph_id: "my-graph".to_string(),
                variant_name: "mobile".to_string(),
                status: LaunchStatus::INITIATED,
                superseded: true,
            }],
        };

        assert_that!(snapshot.poll_outcome()).is_equal_to(SimplePollOutcome::Complete);
    }

    #[rstest]
    #[case::completed(QueryLaunchStatus::LAUNCH_COMPLETED, LaunchStatus::COMPLETED)]
    #[case::failed(QueryLaunchStatus::LAUNCH_FAILED, LaunchStatus::FAILED)]
    #[case::initiated(QueryLaunchStatus::LAUNCH_INITIATED, LaunchStatus::INITIATED)]
    #[case::unrecognized_status_treated_as_failed(
        QueryLaunchStatus::Other("SOME_FUTURE_STATUS".to_string()),
        LaunchStatus::FAILED
    )]
    fn query_launch_status_maps_to_shared_launch_status(
        #[case] status: QueryLaunchStatus,
        #[case] expected: LaunchStatus,
    ) {
        assert_that!(LaunchStatus::from(status)).is_equal_to(expected);
    }
}
