use std::path::PathBuf;

use crate::model::BinaryStatus;

pub fn managed_binary_path() -> Option<PathBuf> {
    None
}

pub fn detect_binary() -> BinaryStatus {
    BinaryStatus::Missing
}

pub fn normalize_local_address(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Please enter a local address or port.".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn extract_tunnel_url(_line: &str) -> Option<String> {
    None
}
