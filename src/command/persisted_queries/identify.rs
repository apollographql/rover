use anyhow::anyhow;
use rover_client::{
    blocking::StudioClient,
    operations::persisted_queries::{
        name::{self, PersistedQueryListNameInput},
        resolve::{self, ResolvePersistedQueryListInput},
    },
};

use crate::{RoverResult, options::OptionalGraphRefOpt};

/// Resolves the graph ID, Persisted Query List ID, and list name a command should
/// operate against, from either a graph ref or an explicit `--graph-id`/`--list-id`
/// pair. Shared by every `persisted-queries` command that identifies a list this way
/// (`publish`, `bulk-delete`).
///
/// `action` is interpolated into the error message shown when the identification is
/// ambiguous or incomplete, e.g. `"publishing operations to"` or `"deleting operations
/// from"`.
pub(crate) async fn identify_persisted_query_list(
    graph: &OptionalGraphRefOpt,
    graph_id: &Option<String>,
    list_id: &Option<String>,
    action: &str,
    client: &StudioClient,
) -> RoverResult<(String, String, String)> {
    match (&graph.graph_ref, graph_id, list_id) {
        (Some(graph_ref), None, None) => {
            let persisted_query_list = resolve::run(
                ResolvePersistedQueryListInput {
                    graph_ref: graph_ref.clone(),
                },
                client,
            )
            .await?;
            Ok((
                graph_ref.graph_id().to_string(),
                persisted_query_list.id,
                persisted_query_list.name,
            ))
        }
        (None, Some(graph_id), Some(list_id)) => {
            let list_name = name::run(
                PersistedQueryListNameInput {
                    graph_id: graph_id.clone(),
                    list_id: list_id.clone(),
                },
                client,
            )
            .await?
            .name;
            Ok((graph_id.to_string(), list_id.to_string(), list_name))
        }
        (None, Some(graph_id), None) => Err(anyhow!(
            "You must specify a --list-id <LIST_ID> when {action} --graph-id {graph_id}, or, if a list is linked to a specific variant, you can leave --graph-id unspecified, and pass a full graph ref as a positional argument."
        )
        .into()),
        (None, None, Some(list_id)) => Err(anyhow!(
            "You must specify a --graph-id <GRAPH_ID> when {action} --list-id {list_id}, or, if {list_id} is linked to a specific variant, you can leave --list-id unspecified, and pass a full graph ref as a positional argument."
        )
        .into()),
        (None, None, None) => Err(anyhow!(
            "You must either specify a <GRAPH_REF> that has a linked persisted query list OR both a --graph-id <GRAPH_ID> and --list-id <LIST_ID>"
        )
        .into()),
        (Some(_), Some(_), Some(_)) | (Some(_), Some(_), None) | (Some(_), None, Some(_)) => {
            unreachable!("clap \"conflicts_with\" should make this impossible to reach")
        }
    }
}
