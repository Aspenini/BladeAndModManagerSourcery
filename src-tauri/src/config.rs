use crate::error::{AppError, AppResult};
use crate::game::paths::normalize_path_string;
use crate::secret::{
    clear_nexus_api_key, has_nexus_api_key, load_nexus_api_key, store_nexus_api_key,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "BladeAndModManagerSourcery";
const CONFIG_FILE: &str = "config.json";
const INSTALLED_FILE: &str = "installed.json";

/// Public app config returned to the frontend. Never includes the raw API key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub game_path: Option<String>,
    pub setup_complete: bool,
    /// Whether a Nexus API key is stored in the OS credential manager.
    #[serde(default)]
    pub has_nexus_api_key: bool,
}

/// On-disk shape. API key is never written here (legacy field only for migration).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredConfig {
    pub game_path: Option<String>,
    pub setup_complete: bool,
    /// Legacy plaintext key — migrated into the OS credential store on load.
    #[serde(default, skip_serializing)]
    nexus_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRecord {
    pub folder_name: String,
    pub nexus_mod_id: Option<u32>,
    pub nexus_file_id: Option<u32>,
    pub mod_name: Option<String>,
    pub version: Option<String>,
    pub installed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstalledIndex {
    pub mods: Vec<InstalledRecord>,
}

pub fn app_data_dir() -> AppResult<PathBuf> {
    let base =
        dirs::data_dir().ok_or_else(|| AppError::msg("Could not resolve app data directory"))?;
    let dir = base.join(APP_DIR_NAME);
    fs::create_dir_all(&dir)?;
    // One-time migrate from the previous app folder name, if present.
    migrate_from_legacy_app_dir(&base, &dir);
    Ok(dir)
}

fn migrate_from_legacy_app_dir(base: &std::path::Path, new_dir: &std::path::Path) {
    let old = base.join("magic-mod-manager");
    if !old.is_dir() || old == new_dir {
        return;
    }
    for name in [CONFIG_FILE, INSTALLED_FILE] {
        let from = old.join(name);
        let to = new_dir.join(name);
        if from.is_file() && !to.exists() {
            let _ = fs::copy(&from, &to);
        }
    }
}

pub fn downloads_dir() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("downloads");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn config_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join(CONFIG_FILE))
}

pub fn installed_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join(INSTALLED_FILE))
}

fn to_public(stored: &StoredConfig) -> AppConfig {
    AppConfig {
        game_path: stored.game_path.clone(),
        setup_complete: stored.setup_complete,
        has_nexus_api_key: has_nexus_api_key(),
    }
}

fn write_stored(stored: &StoredConfig) -> AppResult<()> {
    let path = config_path()?;
    let mut stored = stored.clone();
    if let Some(ref gp) = stored.game_path {
        stored.game_path = Some(normalize_path_string(gp));
    }
    // Never persist the API key to disk.
    stored.nexus_api_key = None;
    let data = serde_json::to_string_pretty(&stored)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn load_config() -> AppResult<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig {
            has_nexus_api_key: has_nexus_api_key(),
            ..AppConfig::default()
        });
    }
    let data = fs::read_to_string(&path)?;
    let mut stored: StoredConfig = serde_json::from_str(&data)?;

    // Migrate legacy plaintext keys into the OS credential store.
    if let Some(legacy) = stored.nexus_api_key.take() {
        let trimmed = legacy.trim().to_string();
        if !trimmed.is_empty() {
            // Prefer existing credential-store entry if already present.
            if load_nexus_api_key()?.is_none() {
                store_nexus_api_key(&trimmed)?;
            }
        }
        // Rewrite config without the plaintext key.
        write_stored(&stored)?;
    }

    // Clean Windows extended paths saved from older canonicalize() results
    if let Some(ref gp) = stored.game_path {
        let cleaned = normalize_path_string(gp);
        if cleaned != *gp {
            stored.game_path = Some(cleaned);
            write_stored(&stored)?;
        }
    }

    Ok(to_public(&stored))
}

pub fn save_config(config: &AppConfig) -> AppResult<()> {
    let stored = StoredConfig {
        game_path: config.game_path.clone(),
        setup_complete: config.setup_complete,
        nexus_api_key: None,
    };
    write_stored(&stored)
}

/// Persist a validated API key in the OS credential store (not in config.json).
pub fn save_nexus_api_key(key: &str) -> AppResult<()> {
    store_nexus_api_key(key)
}

pub fn remove_nexus_api_key() -> AppResult<()> {
    clear_nexus_api_key()
}

/// Load the raw key for Nexus API calls only — never return this to the UI.
pub fn require_nexus_api_key() -> AppResult<String> {
    load_nexus_api_key()?.ok_or_else(|| {
        AppError::msg(
            "Nexus API key not set. Add your personal API key in Settings (from nexusmods.com).",
        )
    })
}

pub fn load_installed() -> AppResult<InstalledIndex> {
    let path = installed_path()?;
    if !path.exists() {
        return Ok(InstalledIndex::default());
    }
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_installed(index: &InstalledIndex) -> AppResult<()> {
    let path = installed_path()?;
    let data = serde_json::to_string_pretty(index)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn upsert_installed(record: InstalledRecord) -> AppResult<()> {
    let mut index = load_installed()?;
    if let Some(existing) = index
        .mods
        .iter_mut()
        .find(|m| m.folder_name.eq_ignore_ascii_case(&record.folder_name))
    {
        *existing = record;
    } else {
        index.mods.push(record);
    }
    save_installed(&index)
}

pub fn remove_installed(folder_name: &str) -> AppResult<()> {
    let mut index = load_installed()?;
    index
        .mods
        .retain(|m| !m.folder_name.eq_ignore_ascii_case(folder_name));
    save_installed(&index)
}
