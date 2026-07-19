/// Errors produced by wingman-core. Messages are user-facing: the Tauri shell
/// forwards them to the frontend via `to_string()`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid panel URL: {0}")]
    InvalidUrl(String),

    #[error("API key is empty or contains invalid characters")]
    InvalidApiKey,

    #[error("invalid server identifier `{0}`")]
    InvalidServerIdentifier(String),

    #[error("authentication failed (HTTP {status}) — check the API key")]
    Unauthorized { status: u16 },

    #[error("panel returned HTTP {status}: {detail}")]
    Api { status: u16, detail: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("could not parse panel response: {0}")]
    Decode(String),

    #[error("websocket error: {0}")]
    Websocket(String),

    #[error("deploy failed: {0}")]
    Deploy(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    Config(String),
}
