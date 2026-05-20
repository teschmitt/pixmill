use std::path::PathBuf;

use ibp_core::Settings;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub settings: Settings,
    pub output_dir: Option<PathBuf>,
}

fn config_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("could not resolve config dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create config dir: {e}"))?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Result<Option<PersistedState>, String> {
    let path = config_file(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("read failed: {e}"))?;
    let parsed: PersistedState =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse failed: {e}"))?;
    Ok(Some(parsed))
}

pub fn save(app: &AppHandle, state: &PersistedState) -> Result<(), String> {
    let path = config_file(app)?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| format!("serialize failed: {e}"))?;
    std::fs::write(&path, bytes).map_err(|e| format!("write failed: {e}"))
}
