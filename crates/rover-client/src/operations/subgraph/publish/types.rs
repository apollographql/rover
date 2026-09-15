use super::{runner::subgraph_publish_mutation, subgraph_publish_launch_status_query};
use crate::shared::{DownstreamLaunch, GitContext, LaunchStatus};

pub(crate) type ResponseData = subgraph_publish_mutation::ResponseData;
pub(crate) type MutationVariables = subgraph_publish_mutation::Variables;
pub(crate) type UpdateResponse =
    subgraph_publish_mutation::SubgraphPublishMutationGraphPublishSubgraph;

use apollo_federation_types::rover::BuildErrors;

type SchemaInput = subgraph_publish_mutation::PartialSchemaInput;
type GitContextInput = subgraph_publish_mutation::GitContextInput;

use rover_studio::types::GraphRef;
use rover_tower::poll_retry::{PollOutcome, SimplePollOutcome};
use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphPublishInput {
    pub graph_ref: GraphRef,
    pub subgraph: String,
    pub url: Option<String>,
    pub schema: String,
    pub git_context: GitContext,
    pub convert_to_federated_graph: bool,
    pub changelog_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SubgraphPublishResponse {
    pub api_schema_hash: Option<String>,

    pub supergraph_was_updated: bool,

    pub subgraph_was_created: bool,

    pub subgraph_was_updated: bool,

    #[serde(skip_serializing)]
    pub build_errors: BuildErrors,

    pub launch_url: Option<String>,

    pub launch_cli_copy: Option<String>,

    /// The publish's own launch's terminal status. `None` when the publish
    /// triggered no launch at all. Carries the outcome as data rather than
    /// as an error, matching `graph publish`'s success-or-failure-is-still-data
    /// approach.
    pub launch_status: Option<LaunchStatus>,

    /// Whether the publish's own launch was superseded by a later one (e.g. a
    /// concurrent publish to the same variant). Always `false` when
    /// `launch_status` is `None`.
    pub launch_superseded: bool,

    pub downstream_launches: Vec<DownstreamLaunch>,
}

/// Input to fetch the status of a launch (and its downstream contract-variant
/// launches) triggered by a `subgraph publish`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaunchStatusInput {
    pub graph_ref: GraphRef,
    pub launch_id: String,
}

impl From<LaunchStatusInput> for subgraph_publish_launch_status_query::Variables {
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
        // be treated as terminal even though it would otherwise look like
        // it's still running -- see `Launch.status`'s doc comment in the
        // schema.
        if (self.status == LaunchStatus::INITIATED && !self.superseded)
            || self
                .downstream_launches
                .iter()
                .any(|launch| launch.status == LaunchStatus::INITIATED && !launch.superseded)
        {
            SimplePollOutcome::Incomplete
        } else {
            SimplePollOutcome::Complete
        }
    }
}

/// The outcome of one poll attempt for a `subgraph publish`'s launch status.
/// Studio's read path can briefly lag behind the mutation that created the
/// launch, so a launch not (yet) existing is a normal, expected response
/// right after the mutation -- not a failure -- and must keep the poll loop
/// going rather than erroring out on the first attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LaunchPoll {
    /// The launch doesn't exist yet from this query's point of view.
    NotFound,
    Found(LaunchSnapshot),
}

impl PollOutcome for LaunchPoll {
    fn poll_outcome(&self) -> SimplePollOutcome {
        match self {
            LaunchPoll::NotFound => SimplePollOutcome::Incomplete,
            LaunchPoll::Found(snapshot) => snapshot.poll_outcome(),
        }
    }
}

type QueryLaunchStatus = subgraph_publish_launch_status_query::LaunchStatus;
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
    subgraph_publish_launch_status_query::SubgraphPublishLaunchStatusQueryGraphVariantLaunch;
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

type QueryDownstreamLaunch = subgraph_publish_launch_status_query::SubgraphPublishLaunchStatusQueryGraphVariantLaunchDownstreamLaunches;
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

impl From<SubgraphPublishInput> for MutationVariables {
    fn from(publish_input: SubgraphPublishInput) -> Self {
        let (graph_id, variant) = publish_input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            subgraph: publish_input.subgraph,
            url: publish_input.url,
            schema: SchemaInput {
                sdl: Some(publish_input.schema),
                hash: None,
            },
            git_context: GitContextInput {
                branch: publish_input.git_context.branch,
                commit: publish_input.git_context.commit,
                committer: publish_input.git_context.author,
                remote_url: publish_input.git_context.remote_url,
                message: publish_input.changelog_message,
            },
            revision: "".to_string(),
        }
    }
}
