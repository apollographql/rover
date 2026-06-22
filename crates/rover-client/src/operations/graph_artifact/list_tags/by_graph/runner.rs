use tower::{Service, ServiceExt};

use super::{
    page_service::ListTagsByGraphPage,
    service::{ListTagsByGraph, ListTagsByGraphRequest},
};
use crate::{
    blocking::StudioClient, operations::graph_artifact::list_tags::types::ListTagsResponse,
    RoverClientError,
};

pub async fn run(
    graph_id: String,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let page_svc = ListTagsByGraphPage::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let mut svc = ListTagsByGraph::new(page_svc);
    let svc = svc.ready().await?;
    svc.call(ListTagsByGraphRequest { graph_id, limit }).await
}
