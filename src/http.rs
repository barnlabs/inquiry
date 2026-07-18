use anyhow::{Context, Result, bail};
use reqwest::Response;
use serde::de::DeserializeOwned;

/// Reads a decoded HTTP response incrementally and refuses it before retaining
/// more than `limit` bytes. This bounds decompressed bodies as well as ordinary
/// responses; `Content-Length` is used only as an early rejection signal.
pub(crate) async fn bytes_limited(
    mut response: Response,
    limit: usize,
    connector: &str,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        bail!("{connector} response exceeded the {limit}-byte safety limit");
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(limit as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("{connector} response stream failed"))?
    {
        if body.len().saturating_add(chunk.len()) > limit {
            bail!("{connector} response exceeded the {limit}-byte safety limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(crate) async fn json_limited<T: DeserializeOwned>(
    response: Response,
    limit: usize,
    connector: &str,
) -> Result<T> {
    let body = bytes_limited(response, limit, connector).await?;
    serde_json::from_slice(&body).with_context(|| format!("{connector} returned invalid JSON"))
}
