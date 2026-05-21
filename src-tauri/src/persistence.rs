use std::path::PathBuf;

use pixmill_core::{Settings, WatchedFolder};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub settings: Settings,
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    pub watched_folders: Vec<WatchedFolder>,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Existing `settings.json` files predate the watch-folder feature and
    /// have no `watchedFolders` field. They must continue to load — the
    /// `#[serde(default)]` attribute is what makes that work.
    #[test]
    fn deserialize_legacy_state_without_watched_folders() {
        let legacy = serde_json::json!({
            "settings": {
                "resize": { "kind": "none" },
                "crop": { "kind": "none" },
                "rotate": "none",
                "outputFormat": "keep",
                "compression": { "kind": "manual" },
                "jpegQuality": 85,
                "webpQuality": 85,
                "preserveExif": true,
            },
            "outputDir": "/tmp/out",
        });
        let parsed: PersistedState =
            serde_json::from_value(legacy).expect("legacy state should deserialize");
        assert!(parsed.watched_folders.is_empty());
        assert_eq!(parsed.output_dir, Some(PathBuf::from("/tmp/out")));
    }
}
