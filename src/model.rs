use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub type TunnelId = u64;
pub type RepaintCallback = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelStatus {
    Starting,
    Running { public_url: String },
    Stopping,
    Stopped,
    Failed(String),
}

impl TunnelStatus {
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running { .. } | Self::Stopping
        )
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn public_url(&self) -> Option<&str> {
        match self {
            Self::Running { public_url } => Some(public_url.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TunnelInfo {
    pub id: TunnelId,
    pub input_address: String,
    pub target_url: String,
    pub status: TunnelStatus,
    pub pid: Option<u32>,
    pub started_at: Instant,
    pub ended_at: Option<Instant>,
    pub recent_logs: Vec<String>,
}

impl TunnelInfo {
    pub fn uptime_display(&self) -> String {
        let elapsed = self
            .ended_at
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.started_at);
        let total_secs = elapsed.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if hours > 0 {
            format!("{hours}h {mins:02}m")
        } else {
            format!("{mins:02}:{secs:02}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinarySource {
    SystemPath,
    ManagedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryStatus {
    Ready {
        path: PathBuf,
        source: BinarySource,
        version: Option<String>,
    },
    Missing,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    DownloadFailed(String),
}

impl BinaryStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::Ready { path, .. } => Some(path),
            _ => None,
        }
    }
}
