use super::*;
use reqwest::{Client, StatusCode, Url, header};
use tokio::io::AsyncSeekExt;

pub(super) fn client() -> Result<Client> {
    Client::builder()
        .user_agent("Starframe packages/1")
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}

fn redirect(source: &Url, location: &str) -> Result<Url> {
    let url = source
        .join(location)
        .map_err(|_| "Package redirect has an invalid URL.")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Package redirect must use HTTPS without credentials or fragments.".into());
    }
    Ok(url)
}

pub(super) async fn download(
    client: &Client,
    artifact: &Artifact,
    file: &mut tokio::fs::File,
    progress: &AtomicU64,
) -> Result<()> {
    let mut error = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(attempt)).await;
        }
        file.set_len(0).await.map_err(|e| e.to_string())?;
        file.rewind().await.map_err(|e| e.to_string())?;
        progress.store(0, Ordering::Relaxed);
        let mut url = Url::parse(&artifact.url).map_err(|_| "Invalid approved artifact URL.")?;
        let mut response = None;
        for hop in 0..=5 {
            match client
                .get(url.clone())
                .header(header::ACCEPT_ENCODING, "identity")
                .send()
                .await
            {
                Ok(reply) if matches!(reply.status().as_u16(), 301 | 302 | 303 | 307 | 308) => {
                    if hop == 5 {
                        return Err(
                            "Package download exceeded five redirects. Contact the curator.".into(),
                        );
                    }
                    let location = reply
                        .headers()
                        .get(header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .ok_or("Package redirect has no valid Location header.")?;
                    url = redirect(&url, location)?;
                }
                Ok(reply) => {
                    response = Some(reply);
                    break;
                }
                Err(_) => {
                    error =
                        "Package transfer failed or timed out. Check your connection and retry."
                            .into();
                    break;
                }
            }
        }
        let Some(mut response) = response else {
            continue;
        };
        if response.status() == StatusCode::TOO_MANY_REQUESTS || response.status().is_server_error()
        {
            error = format!(
                "Download server returned HTTP {}. Retry later.",
                response.status().as_u16()
            );
            if let Some(seconds) = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
            {
                if seconds > 30 {
                    return Err(error);
                }
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_secs(seconds)).await;
                }
            }
            continue;
        }
        if response.status() != StatusCode::OK {
            return Err(format!(
                "Download server returned HTTP {}. Contact the curator if retrying does not help.",
                response.status().as_u16()
            ));
        }
        if response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
        {
            return Err(
                "Download server transformed the artifact. Exact approved bytes are required."
                    .into(),
            );
        }
        if response
            .content_length()
            .is_some_and(|size| size != artifact.size_bytes)
        {
            return Err(
                "Download size differs from the approved artifact. Contact the curator.".into(),
            );
        }
        let mut size = 0u64;
        let mut hash = Sha256::new();
        let mut interrupted = false;
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    interrupted = true;
                    break;
                }
            };
            size = size
                .checked_add(chunk.len() as u64)
                .ok_or("Download size overflow.")?;
            if size > artifact.size_bytes {
                return Err("Download exceeds the approved artifact size.".into());
            }
            file.write_all(&chunk).await.map_err(|e| {
                format!("Cannot save download: {e}. Check free space and app-data permissions.")
            })?;
            hash.update(&chunk);
            progress.store(size, Ordering::Relaxed);
        }
        if interrupted || size != artifact.size_bytes {
            error =
                "Package transfer was interrupted. Retry when your connection is available.".into();
            continue;
        }
        if format!("{:x}", hash.finalize()) != artifact.sha256 {
            return Err("Download SHA-256 differs from the approved artifact. No files were installed; contact the curator.".into());
        }
        file.flush().await.map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_preserve_https_without_credentials() {
        let source =
            Url::parse("https://github.com/author/mod/releases/download/v1/mod.zip").unwrap();
        assert_eq!(
            redirect(
                &source,
                "https://release-assets.githubusercontent.com/asset?token=opaque"
            )
            .unwrap()
            .scheme(),
            "https"
        );
        for target in [
            "http://example.org/a",
            "file:///C:/secret",
            "https://user:pass@example.org/a",
            "https://example.org/a#fragment",
        ] {
            assert!(redirect(&source, target).is_err());
        }
    }
}
