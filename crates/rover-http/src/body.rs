//! Provides utility functions for handling [`Body`] types

use bytes::Bytes;
use http_body::Body;
use http_body_util::BodyExt;

/// Reads a [`Body`] to [`Bytes`]
pub async fn body_to_bytes<B>(body: &mut B) -> Result<Bytes, B::Error>
where
    B: Body<Data = Bytes> + Unpin,
{
    BodyExt::collect(body).await.map(|buf| buf.to_bytes())
}
