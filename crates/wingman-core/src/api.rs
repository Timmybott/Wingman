//! HTTP client for the Pterodactyl client API (`/api/client`).

use crate::error::Error;
use crate::models::{
    ApiList, ApiObject, Backup, FileEntry, PowerSignal, Server, ServerStats, WebsocketDetails,
};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use url::Url;

/// Turn user input like `panel.example.com` into a canonical base URL.
///
/// Adds `https://` when no scheme is given, rejects non-HTTP schemes, strips
/// query/fragment and guarantees a trailing `/` so [`Url::join`] preserves
/// panels hosted under a sub-path.
pub fn normalize_base_url(input: &str) -> Result<Url, Error> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(Error::InvalidUrl("URL is empty".into()));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(Error::InvalidUrl(format!("unsupported scheme `{other}`"))),
    }
    if url.host_str().is_none() {
        return Err(Error::InvalidUrl("URL has no host".into()));
    }
    url.set_query(None);
    url.set_fragment(None);
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

#[derive(Clone)]
pub struct PanelClient {
    base: Url,
    http: reqwest::Client,
}

impl PanelClient {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, Error> {
        let base = normalize_base_url(base_url)?;
        let key = api_key.trim();
        if key.is_empty() {
            return Err(Error::InvalidApiKey);
        }
        let mut auth =
            HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| Error::InvalidApiKey)?;
        auth.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("wingman/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { base, http })
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// Origin of the panel (`scheme://host[:port]`). Wings validates the
    /// `Origin` header on websocket connections against the panel URL.
    pub fn origin(&self) -> String {
        self.base.origin().ascii_serialization()
    }

    /// All servers the API key has access to, following pagination.
    pub async fn list_servers(&self) -> Result<Vec<Server>, Error> {
        let mut page: u64 = 1;
        let mut servers = Vec::new();
        loop {
            let list: ApiList<Server> = self
                .get_json("api/client", &[("page", page.to_string())])
                .await?;
            servers.extend(list.data.into_iter().map(|o| o.attributes));
            match list.meta.map(|m| m.pagination) {
                Some(p) if p.current_page < p.total_pages => page = p.current_page + 1,
                _ => break,
            }
        }
        Ok(servers)
    }

    /// Full details of one server (limits, feature limits, …).
    pub async fn server_details(&self, identifier: &str) -> Result<Server, Error> {
        validate_identifier(identifier)?;
        let server: ApiObject<Server> = self
            .get_json(&format!("api/client/servers/{identifier}"), &[])
            .await?;
        Ok(server.attributes)
    }

    /// All backups of a server.
    pub async fn list_backups(&self, identifier: &str) -> Result<Vec<Backup>, Error> {
        validate_identifier(identifier)?;
        let list: ApiList<Backup> = self
            .get_json(&format!("api/client/servers/{identifier}/backups"), &[])
            .await?;
        Ok(list.data.into_iter().map(|o| o.attributes).collect())
    }

    /// Start a new backup. Creation is asynchronous — poll
    /// [`Self::backup_details`] until `completed_at` is set.
    pub async fn create_backup(&self, identifier: &str, name: &str) -> Result<Backup, Error> {
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/backups"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await?;
        let response = ensure_success(response).await?;
        let bytes = response.bytes().await?;
        let backup: ApiObject<Backup> =
            serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))?;
        Ok(backup.attributes)
    }

    pub async fn backup_details(&self, identifier: &str, uuid: &str) -> Result<Backup, Error> {
        validate_identifier(identifier)?;
        validate_identifier(uuid)?;
        let backup: ApiObject<Backup> = self
            .get_json(
                &format!("api/client/servers/{identifier}/backups/{uuid}"),
                &[],
            )
            .await?;
        Ok(backup.attributes)
    }

    pub async fn delete_backup(&self, identifier: &str, uuid: &str) -> Result<(), Error> {
        validate_identifier(identifier)?;
        validate_identifier(uuid)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/backups/{uuid}"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.delete(url).send().await?;
        ensure_success(response).await?;
        Ok(())
    }

    /// Power state and live resource usage of one server.
    pub async fn server_resources(&self, identifier: &str) -> Result<ServerStats, Error> {
        validate_identifier(identifier)?;
        let stats: ApiObject<ServerStats> = self
            .get_json(&format!("api/client/servers/{identifier}/resources"), &[])
            .await?;
        Ok(stats.attributes)
    }

    /// Send a power signal (start/stop/restart/kill). The panel replies 204.
    pub async fn set_power(&self, identifier: &str, signal: PowerSignal) -> Result<(), Error> {
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/power"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({ "signal": signal.as_str() }))
            .send()
            .await?;
        ensure_success(response).await?;
        Ok(())
    }

    /// Directory listing for the server file browser.
    pub async fn list_files(
        &self,
        identifier: &str,
        directory: &str,
    ) -> Result<Vec<FileEntry>, Error> {
        validate_identifier(identifier)?;
        let list: ApiList<FileEntry> = self
            .get_json(
                &format!("api/client/servers/{identifier}/files/list"),
                &[("directory", directory.to_string())],
            )
            .await?;
        Ok(list.data.into_iter().map(|o| o.attributes).collect())
    }

    /// Raw contents of one server file (`GET .../files/contents`).
    pub async fn read_file(&self, identifier: &str, file: &str) -> Result<Vec<u8>, Error> {
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/files/contents"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.get(url).query(&[("file", file)]).send().await?;
        let response = ensure_success(response).await?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Write raw contents to one server file (`POST .../files/write`).
    pub async fn write_file(
        &self,
        identifier: &str,
        file: &str,
        contents: Vec<u8>,
    ) -> Result<(), Error> {
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/files/write"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self
            .http
            .post(url)
            .query(&[("file", file)])
            .body(contents)
            .send()
            .await?;
        ensure_success(response).await?;
        Ok(())
    }

    /// Compress files/directories under `root` into an archive on the server
    /// (Wings produces a `.tar.gz`). Returns the archive's file entry.
    pub async fn compress_files(
        &self,
        identifier: &str,
        root: &str,
        files: &[String],
    ) -> Result<FileEntry, Error> {
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/files/compress"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({ "root": root, "files": files }))
            .send()
            .await?;
        let response = ensure_success(response).await?;
        let bytes = response.bytes().await?;
        let entry: ApiObject<FileEntry> =
            serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))?;
        Ok(entry.attributes)
    }

    /// Signed one-time URL for downloading a server file.
    pub async fn download_url(&self, identifier: &str, file: &str) -> Result<String, Error> {
        #[derive(Deserialize)]
        struct SignedUrl {
            url: String,
        }
        validate_identifier(identifier)?;
        let url = self
            .base
            .join(&format!("api/client/servers/{identifier}/files/download"))
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.get(url).query(&[("file", file)]).send().await?;
        let response = ensure_success(response).await?;
        let bytes = response.bytes().await?;
        let signed: ApiObject<SignedUrl> =
            serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))?;
        Ok(signed.attributes.url)
    }

    /// Download raw bytes from a signed URL, reporting (received, total).
    pub async fn download_bytes<F>(
        &self,
        signed_url: &str,
        mut on_progress: F,
    ) -> Result<Vec<u8>, Error>
    where
        F: FnMut(u64, u64) + Send,
    {
        let url = Url::parse(signed_url).map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.get(url).send().await?;
        let response = ensure_success(response).await?;
        let total = response.content_length().unwrap_or(0);
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            bytes.extend_from_slice(&chunk);
            on_progress(bytes.len() as u64, total);
        }
        Ok(bytes)
    }

    /// Signed one-time URL for uploading files directly to the Wings node.
    pub async fn upload_url(&self, identifier: &str) -> Result<String, Error> {
        #[derive(Deserialize)]
        struct SignedUrl {
            url: String,
        }
        validate_identifier(identifier)?;
        let obj: ApiObject<SignedUrl> = self
            .get_json(
                &format!("api/client/servers/{identifier}/files/upload"),
                &[],
            )
            .await?;
        Ok(obj.attributes.url)
    }

    /// Upload a zip archive to a signed URL, streamed from disk. `directory`
    /// is the remote target (e.g. `/` or `/app`). `on_progress` receives
    /// (bytes_sent, bytes_total) as the stream is consumed.
    pub async fn upload_zip<F>(
        &self,
        signed_url: &str,
        directory: &str,
        archive: &std::path::Path,
        remote_name: &str,
        mut on_progress: F,
    ) -> Result<(), Error>
    where
        F: FnMut(u64, u64) + Send + 'static,
    {
        let mut url = Url::parse(signed_url).map_err(|e| Error::InvalidUrl(e.to_string()))?;
        url.query_pairs_mut().append_pair("directory", directory);

        let file = tokio::fs::File::open(archive).await?;
        let total = file.metadata().await?.len();
        let mut sent: u64 = 0;
        let stream = tokio_util::io::ReaderStream::new(file).inspect(move |chunk| {
            if let Ok(chunk) = chunk {
                sent += chunk.len() as u64;
                on_progress(sent, total);
            }
        });
        let part =
            reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), total)
                .file_name(remote_name.to_string())
                .mime_str("application/zip")
                .map_err(|e| Error::Deploy(e.to_string()))?;
        let form = reqwest::multipart::Form::new().part("files", part);

        let response = self.http.post(url).multipart(form).send().await?;
        ensure_success(response).await?;
        Ok(())
    }

    /// Unpack an archive on the server. `root` is the directory containing
    /// `file`; entries are extracted into `root`.
    pub async fn decompress_file(
        &self,
        identifier: &str,
        root: &str,
        file: &str,
    ) -> Result<(), Error> {
        validate_identifier(identifier)?;
        self.post_json(
            &format!("api/client/servers/{identifier}/files/decompress"),
            &serde_json::json!({ "root": root, "file": file }),
        )
        .await
    }

    /// Delete files or directories, paths relative to `root`.
    pub async fn delete_files(
        &self,
        identifier: &str,
        root: &str,
        files: &[String],
    ) -> Result<(), Error> {
        validate_identifier(identifier)?;
        self.post_json(
            &format!("api/client/servers/{identifier}/files/delete"),
            &serde_json::json!({ "root": root, "files": files }),
        )
        .await
    }

    /// Create one directory level under `root`.
    pub async fn create_folder(
        &self,
        identifier: &str,
        root: &str,
        name: &str,
    ) -> Result<(), Error> {
        validate_identifier(identifier)?;
        self.post_json(
            &format!("api/client/servers/{identifier}/files/create-folder"),
            &serde_json::json!({ "root": root, "name": name }),
        )
        .await
    }

    /// Credentials for the console/stats websocket on the Wings node.
    /// Tokens are short-lived (~10–15 min); fetch a fresh one to re-auth.
    pub async fn websocket_details(&self, identifier: &str) -> Result<WebsocketDetails, Error> {
        #[derive(serde::Deserialize)]
        struct Envelope {
            data: WebsocketDetails,
        }
        validate_identifier(identifier)?;
        let envelope: Envelope = self
            .get_json(&format!("api/client/servers/{identifier}/websocket"), &[])
            .await?;
        Ok(envelope.data)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let url = self
            .base
            .join(path)
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.get(url).query(query).send().await?;
        let response = ensure_success(response).await?;
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))
    }

    /// POST a JSON body to an endpoint that answers 204 No Content.
    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<(), Error> {
        let url = self
            .base
            .join(path)
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        let response = self.http.post(url).json(body).send().await?;
        ensure_success(response).await?;
        Ok(())
    }
}

async fn ensure_success(response: reqwest::Response) -> Result<reqwest::Response, Error> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(Error::Unauthorized {
            status: status.as_u16(),
        });
    }
    if !status.is_success() {
        return Err(Error::Api {
            status: status.as_u16(),
            detail: extract_api_error(response).await,
        });
    }
    Ok(response)
}

/// Identifiers are used to build URL paths — restrict them to the panel's
/// alphabet so a malformed value can never alter the request path.
fn validate_identifier(identifier: &str) -> Result<(), Error> {
    let valid = !identifier.is_empty()
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidServerIdentifier(identifier.into()))
    }
}

/// Pterodactyl error bodies look like `{"errors":[{"code":…,"status":…,"detail":…}]}`.
async fn extract_api_error(response: reqwest::Response) -> String {
    #[derive(Deserialize)]
    struct Body {
        errors: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        code: Option<String>,
        detail: Option<String>,
    }
    match response.json::<Body>().await {
        Ok(body) if !body.errors.is_empty() => body
            .errors
            .into_iter()
            .map(|item| match (item.code, item.detail) {
                (Some(code), Some(detail)) if !detail.is_empty() => format!("{code}: {detail}"),
                (Some(code), _) => code,
                (None, Some(detail)) => detail,
                (None, None) => "unknown error".into(),
            })
            .collect::<Vec<_>>()
            .join("; "),
        _ => "unexpected response from panel".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_when_scheme_is_missing() {
        let url = normalize_base_url("panel.example.com").unwrap();
        assert_eq!(url.as_str(), "https://panel.example.com/");
    }

    #[test]
    fn keeps_sub_paths_and_appends_slash() {
        let url = normalize_base_url("https://example.com/panel").unwrap();
        assert_eq!(url.as_str(), "https://example.com/panel/");
        assert_eq!(
            url.join("api/client").unwrap().as_str(),
            "https://example.com/panel/api/client"
        );
    }

    #[test]
    fn strips_query_fragment_and_trailing_slashes() {
        let url = normalize_base_url("https://example.com/?tab=1#top").unwrap();
        assert_eq!(url.as_str(), "https://example.com/");
    }

    #[test]
    fn rejects_empty_and_non_http_urls() {
        assert!(matches!(
            normalize_base_url("  "),
            Err(Error::InvalidUrl(_))
        ));
        assert!(matches!(
            normalize_base_url("ftp://example.com"),
            Err(Error::InvalidUrl(_))
        ));
    }

    #[test]
    fn rejects_empty_api_key() {
        assert!(matches!(
            PanelClient::new("https://example.com", "  "),
            Err(Error::InvalidApiKey)
        ));
    }

    #[test]
    fn rejects_path_traversal_in_identifier() {
        assert!(validate_identifier("abc123-def").is_ok());
        assert!(validate_identifier("../admin").is_err());
        assert!(validate_identifier("").is_err());
    }
}
