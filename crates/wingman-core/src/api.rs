//! HTTP client for the Pterodactyl client API (`/api/client`).

use crate::error::Error;
use crate::models::{ApiList, ApiObject, Server, ServerStats};
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

    /// Power state and live resource usage of one server.
    pub async fn server_resources(&self, identifier: &str) -> Result<ServerStats, Error> {
        validate_identifier(identifier)?;
        let stats: ApiObject<ServerStats> = self
            .get_json(&format!("api/client/servers/{identifier}/resources"), &[])
            .await?;
        Ok(stats.attributes)
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
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))
    }
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
