use std::path::PathBuf;

use crate::model::{BinaryStatus, RepaintCallback, TunnelId, TunnelInfo};

pub struct TunnelManager {
    _on_update: Option<RepaintCallback>,
}

impl TunnelManager {
    pub fn new(on_update: Option<RepaintCallback>) -> Self {
        Self {
            _on_update: on_update,
        }
    }

    pub fn with_binary_path(_path: PathBuf, on_update: Option<RepaintCallback>) -> Self {
        Self {
            _on_update: on_update,
        }
    }

    pub fn binary_status(&self) -> BinaryStatus {
        BinaryStatus::Missing
    }

    pub fn refresh_binary_status(&self) {}

    pub fn start_binary_download(&self) {}

    pub fn start_tunnel(&self, _raw_address: &str) -> Result<TunnelId, String> {
        Err("Not implemented yet".to_string())
    }

    pub fn stop_tunnel(&self, _id: TunnelId) -> Result<(), String> {
        Ok(())
    }

    pub fn stop_all(&self) {}

    pub fn remove_tunnel(&self, _id: TunnelId) {}

    pub fn clear_inactive(&self) {}

    pub fn tunnels(&self) -> Vec<TunnelInfo> {
        Vec::new()
    }

    pub fn active_count(&self) -> usize {
        0
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}
