//! Typed subset of the Pterodactyl client API (`/api/client`) responses.

use serde::{Deserialize, Serialize};

/// Generic list envelope: `{"object":"list","data":[{"object":…,"attributes":…}],"meta":…}`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiList<T> {
    pub data: Vec<ApiObject<T>>,
    #[serde(default)]
    pub meta: Option<ListMeta>,
}

/// Generic single-object envelope: `{"object":…,"attributes":…}`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiObject<T> {
    pub attributes: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListMeta {
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    pub total: u64,
    pub count: u64,
    pub per_page: u64,
    pub current_page: u64,
    pub total_pages: u64,
}

/// A server as returned by `GET /api/client`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    /// Short identifier used in API paths (e.g. `d3aac109`).
    pub identifier: String,
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub node: String,
    pub limits: ServerLimits,
    pub feature_limits: FeatureLimits,
    /// Panel-side status: `None` when normal, otherwise e.g. "suspended" or "installing".
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub is_suspended: bool,
    #[serde(default)]
    pub is_installing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLimits {
    /// MiB, 0 = unlimited.
    pub memory: i64,
    pub swap: i64,
    /// MiB, 0 = unlimited.
    pub disk: i64,
    #[serde(default)]
    pub io: Option<i64>,
    /// Percent across cores (100 = one full core), 0 = unlimited.
    pub cpu: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureLimits {
    pub databases: i64,
    pub allocations: i64,
    /// Backup slots on the server — relevant for pre-deploy backup rotation (M4).
    pub backups: i64,
}

/// Response of `GET /api/client/servers/{id}/resources`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStats {
    pub current_state: PowerState,
    #[serde(default)]
    pub is_suspended: bool,
    pub resources: ResourceUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerState {
    Running,
    Starting,
    Stopping,
    Offline,
}

impl PowerState {
    /// Parse the state strings Wings uses in websocket `status` events.
    pub fn parse_wings(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "starting" => Some(Self::Starting),
            "stopping" => Some(Self::Stopping),
            "offline" => Some(Self::Offline),
            _ => None,
        }
    }
}

/// Power signal for `POST /api/client/servers/{id}/power`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerSignal {
    Start,
    Stop,
    Restart,
    Kill,
}

impl PowerSignal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Kill => "kill",
        }
    }
}

impl std::str::FromStr for PowerSignal {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            "restart" => Ok(Self::Restart),
            "kill" => Ok(Self::Kill),
            other => Err(format!("unknown power signal `{other}`")),
        }
    }
}

/// Response of `GET /api/client/servers/{id}/websocket`: credentials for the
/// console/stats websocket on the Wings node.
#[derive(Debug, Clone, Deserialize)]
pub struct WebsocketDetails {
    pub token: String,
    pub socket: String,
}

/// A server backup as returned by the backups endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub is_successful: bool,
    #[serde(default)]
    pub created_at: String,
    /// `None` while the backup is still being taken.
    #[serde(default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_bytes: u64,
    /// Percent across cores, same scale as `ServerLimits::cpu`.
    pub cpu_absolute: f64,
    pub disk_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    /// Milliseconds since the server process started.
    pub uptime: u64,
}
