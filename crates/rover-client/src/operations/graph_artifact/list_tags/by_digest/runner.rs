use tower::{Service, ServiceExt};

use super::{
    page_service::ListTagsByDigestPage,
    service::{ListTagsByDigest, ListTagsByDigestRequest},
};
use crate::{
    blocking::StudioClient, operations::graph_artifact::list_tags::types::ListTagsResponse,
    RoverClientError,
};

pub async fn run(
    graph_id: String,
    digest: String,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let page_svc = ListTagsByDigestPage::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let mut svc = ListTagsByDigest::new(page_svc);
    let svc = svc.ready().await?;
    svc.call(ListTagsByDigestRequest {
        graph_id,
        digest,
        limit,
    })
    .await
}
