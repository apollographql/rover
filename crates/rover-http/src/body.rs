use bytes::Bytes;
use http_body::Body;
use http_body_util::BodyExt;

pub async fn body_to_bytes<B>(body: &mut B) -> Result<Vec<u8>, B::Error>
where
    B: Body<Data = Bytes> + Unpin,
{
    let mut bytes = Vec::new();
    while let Some(next) = body.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            bytes.extend_from_slice(chunk);
        }
    }
    Ok(bytes)
}
