use reqwest::header::HeaderValue;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::fmt;
use std::path::Path;

const LOGIN_PATH: &str = "/api/v2/auth/login";
const ADD_PATH: &str = "/api/v2/torrents/add";
const INFO_PATH: &str = "/api/v2/torrents/info";
const TRANSFER_PATH: &str = "/api/v2/transfer/info";

#[derive(Debug)]
pub enum QbittorrentError {
    Http(reqwest::Error),
    InvalidHeader(String),
    AuthFailed(String),
    InvalidInput(String),
    Io(std::io::Error),
}

impl fmt::Display for QbittorrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QbittorrentError::Http(err) => write!(f, "http error: {}", err),
            QbittorrentError::InvalidHeader(message) => write!(f, "invalid header: {}", message),
            QbittorrentError::AuthFailed(message) => write!(f, "auth failed: {}", message),
            QbittorrentError::InvalidInput(message) => write!(f, "invalid input: {}", message),
            QbittorrentError::Io(err) => write!(f, "io error: {}", err),
        }
    }
}

impl std::error::Error for QbittorrentError {}

impl From<reqwest::Error> for QbittorrentError {
    fn from(err: reqwest::Error) -> Self {
        QbittorrentError::Http(err)
    }
}

impl From<std::io::Error> for QbittorrentError {
    fn from(err: std::io::Error) -> Self {
        QbittorrentError::Io(err)
    }
}

#[derive(Debug, Clone)]
pub struct QbittorrentClient {
    base_url: String,
    username: Option<String>,
    password: Option<String>,
    client: reqwest::Client,
}

impl QbittorrentClient {
    pub fn new(
        base_url: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, QbittorrentError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static("Anicargo-qBittorrent/0.1"),
        );

        let client = reqwest::Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .build()
            .map_err(QbittorrentError::Http)?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            username,
            password,
            client,
        })
    }

    pub async fn add_magnet(
        &self,
        magnet: &str,
        save_path: Option<&str>,
    ) -> Result<(), QbittorrentError> {
        let magnet = magnet.trim();
        if magnet.is_empty() {
            return Err(QbittorrentError::InvalidInput(
                "magnet is empty".to_string(),
            ));
        }

        self.login().await?;
        let url = format!("{}{}", self.base_url, ADD_PATH);
        let mut form = vec![("urls".to_string(), magnet.to_string())];
        if let Some(path) = save_path {
            form.push(("savepath".to_string(), path.to_string()));
        }

        let response = self.client.post(url).form(&form).send().await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() || !body.to_lowercase().contains("ok") {
            return Err(QbittorrentError::AuthFailed(body));
        }

        Ok(())
    }

    pub async fn add_torrent_bytes(
        &self,
        filename: &str,
        bytes: Vec<u8>,
        save_path: Option<&str>,
    ) -> Result<(), QbittorrentError> {
        if bytes.is_empty() {
            return Err(QbittorrentError::InvalidInput(
                "torrent file is empty".to_string(),
            ));
        }

        self.login().await?;
        let url = format!("{}{}", self.base_url, ADD_PATH);
        let part = Part::bytes(bytes).file_name(filename.to_string());
        let mut form = Form::new().part("torrents", part);
        if let Some(path) = save_path {
            form = form.text("savepath", path.to_string());
        }

        let response = self.client.post(url).multipart(form).send().await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() || !body.to_lowercase().contains("ok") {
            return Err(QbittorrentError::AuthFailed(body));
        }

        Ok(())
    }

    pub async fn add_torrent_file(
        &self,
        path: &Path,
        save_path: Option<&str>,
    ) -> Result<(), QbittorrentError> {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| QbittorrentError::InvalidInput("invalid torrent filename".to_string()))?;
        let bytes = std::fs::read(path)?;
        self.add_torrent_bytes(filename, bytes, save_path).await
    }

    pub async fn list_completed(&self) -> Result<Vec<TorrentInfo>, QbittorrentError> {
        self.login().await?;
        let url = format!("{}{}", self.base_url, INFO_PATH);
        let response = self
            .client
            .get(url)
            .query(&[("filter", "completed")])
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<Vec<TorrentInfo>>().await?)
    }

    pub async fn transfer_info(&self) -> Result<TransferInfo, QbittorrentError> {
        self.login().await?;
        let url = format!("{}{}", self.base_url, TRANSFER_PATH);
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<TransferInfo>().await?)
    }

    async fn login(&self) -> Result<(), QbittorrentError> {
        let (Some(username), Some(password)) = (&self.username, &self.password) else {
            return Ok(());
        };

        let url = format!("{}{}", self.base_url, LOGIN_PATH);
        let response = self
            .client
            .post(url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() || !body.to_lowercase().contains("ok") {
            return Err(QbittorrentError::AuthFailed(body));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TorrentInfo {
    pub hash: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub save_path: Option<String>,
    #[serde(default)]
    pub content_path: Option<String>,
    #[serde(default)]
    pub completion_on: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TransferInfo {
    #[serde(default, rename = "dl_info_speed")]
    pub download_speed_bytes: u64,
    #[serde(default, rename = "up_info_speed")]
    pub upload_speed_bytes: u64,
    #[serde(default, rename = "dl_info_data")]
    pub download_total_bytes: u64,
    #[serde(default, rename = "up_info_data")]
    pub upload_total_bytes: u64,
    #[serde(default, rename = "dl_rate_limit")]
    pub download_rate_limit: i64,
    #[serde(default, rename = "up_rate_limit")]
    pub upload_rate_limit: i64,
    #[serde(default)]
    pub dht_nodes: i64,
    #[serde(default)]
    pub connection_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_empty_magnet() {
        let client = QbittorrentClient::new("http://127.0.0.1:8080".to_string(), None, None)
            .expect("client");
        let err = client.add_magnet("  ", None).await.unwrap_err();
        assert!(matches!(err, QbittorrentError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn rejects_empty_torrent() {
        let client = QbittorrentClient::new("http://127.0.0.1:8080".to_string(), None, None)
            .expect("client");
        let err = client
            .add_torrent_bytes("file.torrent", Vec::new(), None)
            .await
            .unwrap_err();
        assert!(matches!(err, QbittorrentError::InvalidInput(_)));
    }
}
