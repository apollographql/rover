use std::{future::Future, pin::Pin};

use tower::{Service, ServiceExt};

use super::page_service::{ListTagsByDigestPageRequest, ListTagsByDigestPageResponse};
use crate::{
    operations::graph_artifact::list_tags::types::{ListTagEntry, ListTagsResponse},
    RoverClientError,
};

pub(super) struct ListTagsByDigestRequest {
    pub graph_id: String,
    pub digest: String,
    pub limit: usize,
}

/// Outer service: drives the pagination loop, calling the inner page service
/// repeatedly until all tags are collected or the limit is reached.
#[derive(Clone)]
pub(super) struct ListTagsByDigest<S: Clone> {
    inner: S,
}

impl<S: Clone> ListTagsByDigest<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ListTagsByDigestRequest> for ListTagsByDigest<S>
where
    S: Service<
            ListTagsByDigestPageRequest,
            Response = ListTagsByDigestPageResponse,
            Error = RoverClientError,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = ListTagsResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::<ListTagsByDigestPageRequest>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: ListTagsByDigestRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let mut tags: Vec<ListTagEntry> = Vec::new();
            let mut after: Option<String> = None;

            loop {
                let page = inner
                    .ready()
                    .await?
                    .call(ListTagsByDigestPageRequest {
                        graph_id: req.graph_id.clone(),
                        digest: req.digest.clone(),
                        after: after.clone(),
                    })
                    .await?;

                let next_cursor = page.end_cursor.clone();
                let has_next = page.has_next_page;
                tags.extend(page.tags);

                if super::super::reached_limit(&mut tags, req.limit) {
                    break;
                }

                match (has_next, next_cursor) {
                    (true, Some(cursor)) if after.as_deref() != Some(&cursor) => {
                        after = Some(cursor);
                    }
                    _ => break,
                }
            }

            Ok(ListTagsResponse { tags })
        };
        Box::pin(fut)
    }
}
