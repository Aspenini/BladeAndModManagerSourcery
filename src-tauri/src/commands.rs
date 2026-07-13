use crate::config::{load_config, save_config, AppConfig};
use crate::error::{AppError, AppResult};
use crate::game::detect::{detect_game, inspect_path, DetectedGame};
use crate::game::paths::{ensure_mods_dir, resolve_paths, GamePaths};
use crate::config::BoxesFile;
use crate::mods::boxes::{self, ActivateReport};
use crate::mods::install::{install_from_archive, set_mod_enabled, uninstall_mod};
use crate::mods::scan::{list_installed_mods, LocalMod};
use crate::nexus::client::{
    nexus_file_page_url, nexus_mod_page_url, DownloadLink, NexusClient, NexusFile, NexusModDetail,
    NexusModSummary, NexusUser, GAME_DOMAIN,
};
use crate::nexus::nxm::{parse_nxm_url, NxmLink};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Prevents double-handling when cold-start get_current + on_open_url both fire
/// for the same NXM URL (or a second instance forwards the same link).
static NXM_RECENT: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());
static NXM_IN_FLIGHT: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn nxm_claim(dedupe_key: &str) -> bool {
    const WINDOW: Duration = Duration::from_secs(120);
    let now = Instant::now();
    {
        let mut recent = NXM_RECENT.lock().unwrap_or_else(|e| e.into_inner());
        recent.retain(|(_, t)| now.duration_since(*t) < WINDOW);
        if recent.iter().any(|(k, _)| k == dedupe_key) {
            return false;
        }
    }

    let mut guard = NXM_IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
    let set = guard.get_or_insert_with(HashSet::new);
    if !set.insert(dedupe_key.to_string()) {
        return false;
    }
    true
}

fn nxm_release(dedupe_key: &str, succeeded: bool) {
    {
        let mut guard = NXM_IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(set) = guard.as_mut() {
            set.remove(dedupe_key);
        }
    }
    // Remember successful tickets so a lagging duplicate delivery is ignored.
    // Failed installs stay retryable (same key rarely works twice on Nexus anyway).
    if succeeded {
        let mut recent = NXM_RECENT.lock().unwrap_or_else(|e| e.into_inner());
        recent.push((dedupe_key.to_string(), Instant::now()));
    }
}

fn nxm_dedupe_key(link: &NxmLink) -> String {
    format!(
        "{}:{}:{}:{}",
        link.game_domain, link.mod_id, link.file_id, link.key
    )
}

fn require_game_path(config: &AppConfig) -> AppResult<PathBuf> {
    config
        .game_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::msg("Game path is not configured. Complete setup first."))
}

#[tauri::command]
pub fn get_config() -> AppResult<AppConfig> {
    load_config()
}

#[tauri::command]
pub fn save_app_config(config: AppConfig) -> AppResult<AppConfig> {
    save_config(&config)?;
    Ok(config)
}

#[tauri::command]
pub fn detect_game_install() -> AppResult<Option<DetectedGame>> {
    detect_game()
}

#[tauri::command]
pub fn validate_and_inspect_path(path: String) -> AppResult<DetectedGame> {
    inspect_path(Path::new(&path))
}

#[tauri::command]
pub fn confirm_game_path(path: String) -> AppResult<AppConfig> {
    let detected = inspect_path(Path::new(&path))?;
    ensure_mods_dir(Path::new(&detected.path))?;

    let mut config = load_config()?;
    config.game_path = Some(detected.path);
    config.setup_complete = true;
    save_config(&config)?;
    Ok(config)
}

#[tauri::command]
pub fn get_game_paths() -> AppResult<GamePaths> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    resolve_paths(&game)
}

#[tauri::command]
pub fn list_mods() -> AppResult<Vec<LocalMod>> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    list_installed_mods(&game)
}

#[tauri::command]
pub fn toggle_mod(folder_name: String, enabled: bool) -> AppResult<String> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    set_mod_enabled(&game, &folder_name, enabled)
}

#[tauri::command]
pub fn remove_mod(folder_name: String) -> AppResult<()> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    uninstall_mod(&game, &folder_name)
}

#[tauri::command]
pub fn list_boxes() -> AppResult<BoxesFile> {
    boxes::get_boxes()
}

#[tauri::command]
pub fn create_box(name: String) -> AppResult<BoxesFile> {
    boxes::create_box(&name)
}

#[tauri::command]
pub fn rename_box(box_id: String, name: String) -> AppResult<BoxesFile> {
    boxes::rename_box(&box_id, &name)
}

#[tauri::command]
pub fn delete_box(box_id: String) -> AppResult<BoxesFile> {
    boxes::delete_box(&box_id)
}

#[tauri::command]
pub fn assign_mod_box(folder_name: String, box_id: Option<String>) -> AppResult<BoxesFile> {
    boxes::assign_mod(&folder_name, box_id.as_deref())
}

#[tauri::command]
pub fn activate_box(box_id: String) -> AppResult<ActivateReport> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    boxes::activate_box(&game, &box_id)
}

#[tauri::command]
pub fn clear_active_box() -> AppResult<BoxesFile> {
    boxes::clear_active_box()
}

#[tauri::command]
pub fn import_mod_archive(archive_path: String) -> AppResult<String> {
    let config = load_config()?;
    let game = require_game_path(&config)?;
    let path = PathBuf::from(&archive_path);
    if !path.exists() {
        return Err(AppError::msg("Archive file does not exist"));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "zip" {
        return Err(AppError::msg(
            "Only .zip archives are supported for import right now.",
        ));
    }
    install_from_archive(&game, &path, None, None, None, None, None)
}

#[tauri::command]
pub async fn nexus_validate(api_key: String) -> AppResult<NexusUser> {
    let client = NexusClient::new(api_key)?;
    client.validate_key().await
}

#[tauri::command]
pub async fn nexus_save_api_key(api_key: String) -> AppResult<NexusUser> {
    let client = NexusClient::new(&api_key)?;
    let user = client.validate_key().await?;
    crate::config::save_nexus_api_key(api_key.trim())?;
    Ok(user)
}

#[tauri::command]
pub fn nexus_clear_api_key() -> AppResult<()> {
    crate::config::remove_nexus_api_key()
}

/// Validate the stored API key and return the signed-in user (including Premium flag).
#[tauri::command]
pub async fn nexus_get_user() -> AppResult<NexusUser> {
    let client = nexus_from_config()?;
    client.validate_key().await
}

fn nexus_from_config() -> AppResult<NexusClient> {
    let key = crate::config::require_nexus_api_key()?;
    NexusClient::new(key)
}

#[tauri::command]
pub async fn nexus_list_mods(
    sort: String,
    query: Option<String>,
) -> AppResult<Vec<NexusModSummary>> {
    let client = nexus_from_config()?;
    if let Some(q) = query {
        if !q.trim().is_empty() {
            return client.search(&q).await;
        }
    }
    match sort.as_str() {
        "latest" => client.latest_added().await,
        "updated" => client.latest_updated().await,
        _ => client.trending().await,
    }
}

#[tauri::command]
pub async fn nexus_mod_detail(mod_id: u32) -> AppResult<NexusModDetail> {
    let client = nexus_from_config()?;
    client.mod_detail(mod_id).await
}

#[tauri::command]
pub async fn nexus_mod_files(mod_id: u32) -> AppResult<Vec<NexusFile>> {
    let client = nexus_from_config()?;
    client.list_files(mod_id).await
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub folder_name: String,
    pub message: String,
    pub mod_id: Option<u32>,
    pub file_id: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NxmProgressEvent {
    pub stage: String,
    pub mod_id: u32,
    pub file_id: u32,
    pub message: String,
    pub folder_name: Option<String>,
}

/// Premium (or ticketed free) download + install.
async fn download_and_install(
    game_domain: &str,
    mod_id: u32,
    file_id: u32,
    mod_name: Option<&str>,
    file_version: Option<&str>,
    free_ticket: Option<(&str, u64)>,
) -> AppResult<InstallResult> {
    crate::log::info(format!(
        "download_and_install domain={game_domain} mod={mod_id} file={file_id} ticketed={}",
        free_ticket.is_some()
    ));

    let config = load_config()?;
    let game = require_game_path(&config)?;
    let client = nexus_from_config()?;

    let links: Vec<DownloadLink> = client
        .download_links(game_domain, mod_id, file_id, free_ticket)
        .await?;
    let uri = links
        .first()
        .map(|l| l.uri.clone())
        .ok_or_else(|| AppError::msg("No download URI available"))?;

    let downloads = crate::config::downloads_dir()?;
    let file_name = format!("nexus_{mod_id}_{file_id}.zip");
    let dest = downloads.join(&file_name);

    client.download_file_to(&uri, &dest).await?;

    // If CDN returned non-zip (HTML gate), surface a helpful error
    let is_zip = {
        let bytes = std::fs::read(&dest).map_err(|e| {
            crate::log::io_ctx(&dest, "reading downloaded file to verify ZIP magic", e)
        })?;
        bytes.len() >= 4 && &bytes[0..2] == b"PK"
    };
    if !is_zip {
        crate::log::error(format!(
            "Downloaded file is not a ZIP: {}",
            dest.display()
        ));
        let _ = std::fs::remove_file(&dest);
        return Err(AppError::msg(format!(
            "Download did not return a ZIP archive. Open the mod page and try again: {}",
            nexus_mod_page_url(mod_id)
        )));
    }

    let folder = install_from_archive(
        &game,
        &dest,
        mod_name,
        Some(mod_id),
        Some(file_id),
        mod_name,
        file_version,
    )?;

    Ok(InstallResult {
        folder_name: folder,
        message: "Mod installed successfully.".into(),
        mod_id: Some(mod_id),
        file_id: Some(file_id),
    })
}

#[tauri::command]
pub async fn nexus_download_and_install(
    mod_id: u32,
    file_id: u32,
    mod_name: Option<String>,
    file_version: Option<String>,
) -> AppResult<InstallResult> {
    download_and_install(
        GAME_DOMAIN,
        mod_id,
        file_id,
        mod_name.as_deref(),
        file_version.as_deref(),
        None,
    )
    .await
}

/// Install from a free-user NXM ticket (key + expires from Mod Manager Download).
#[tauri::command]
pub async fn nexus_download_with_nxm(
    nxm_url: String,
    mod_name: Option<String>,
    file_version: Option<String>,
) -> AppResult<InstallResult> {
    let link = parse_nxm_url(&nxm_url)?;
    install_from_nxm_link(&link, mod_name.as_deref(), file_version.as_deref()).await
}

async fn install_from_nxm_link(
    link: &NxmLink,
    mod_name: Option<&str>,
    file_version: Option<&str>,
) -> AppResult<InstallResult> {
    if link.is_expired() {
        return Err(AppError::msg(
            "This NXM download link has expired. Open the file on Nexus again and choose Mod Manager Download → Slow Download.",
        ));
    }

    // Prefer resolving a display name when the caller did not provide one.
    let mut resolved_name = mod_name.map(|s| s.to_string());
    let mut resolved_version = file_version.map(|s| s.to_string());
    if resolved_name.is_none() || resolved_version.is_none() {
        if let Ok(client) = nexus_from_config() {
            if resolved_name.is_none() {
                if let Ok(detail) = client.mod_detail(link.mod_id).await {
                    resolved_name = Some(detail.name);
                }
            }
            if resolved_version.is_none() {
                if let Ok(files) = client.list_files(link.mod_id).await {
                    if let Some(f) = files.iter().find(|f| f.file_id == link.file_id) {
                        resolved_version = f.version.clone();
                        if resolved_name.is_none() {
                            // Keep mod name preference; file name is less ideal as folder label.
                        }
                    }
                }
            }
        }
    }

    download_and_install(
        &link.game_domain,
        link.mod_id,
        link.file_id,
        resolved_name.as_deref(),
        resolved_version.as_deref(),
        Some((link.key.as_str(), link.expires)),
    )
    .await
}

/// Handle one or more deep-link URLs (typically a single `nxm://…`).
/// Emits `nxm-download` progress events for the UI and installs automatically.
pub fn handle_nxm_urls(app: &AppHandle, urls: Vec<String>) {
    for url in urls {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.to_ascii_lowercase().starts_with("nxm://") {
            crate::log::warn(format!("Ignored non-NXM deep link: {trimmed}"));
            let _ = app.emit(
                "nxm-download",
                NxmProgressEvent {
                    stage: "error".into(),
                    mod_id: 0,
                    file_id: 0,
                    message: format!("Ignored non-NXM deep link: {trimmed}"),
                    folder_name: None,
                },
            );
            continue;
        }

        let link = match parse_nxm_url(&trimmed) {
            Ok(l) => l,
            Err(e) => {
                crate::log::error(format!("Bad NXM URL: {e}"));
                let _ = app.emit(
                    "nxm-download",
                    NxmProgressEvent {
                        stage: "error".into(),
                        mod_id: 0,
                        file_id: 0,
                        message: e.to_string(),
                        folder_name: None,
                    },
                );
                continue;
            }
        };

        let dedupe = nxm_dedupe_key(&link);
        if !nxm_claim(&dedupe) {
            crate::log::info(format!(
                "Skipping duplicate NXM delivery for mod {} file {} (already in flight / recent)",
                link.mod_id, link.file_id
            ));
            continue;
        }

        // Bring the main window forward so the user sees progress.
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::log::info(format!(
                "NXM install started mod={} file={} expires={}",
                link.mod_id, link.file_id, link.expires
            ));

            let _ = app_handle.emit(
                "nxm-download",
                NxmProgressEvent {
                    stage: "started".into(),
                    mod_id: link.mod_id,
                    file_id: link.file_id,
                    message: format!(
                        "Downloading mod {} file {} via Nexus (free / slow)…",
                        link.mod_id, link.file_id
                    ),
                    folder_name: None,
                },
            );

            let succeeded = match install_from_nxm_link(&link, None, None).await {
                Ok(result) => {
                    crate::log::info(format!(
                        "NXM install finished mod={} file={} folder={}",
                        link.mod_id, link.file_id, result.folder_name
                    ));
                    let _ = app_handle.emit(
                        "nxm-download",
                        NxmProgressEvent {
                            stage: "finished".into(),
                            mod_id: link.mod_id,
                            file_id: link.file_id,
                            message: result.message.clone(),
                            folder_name: Some(result.folder_name),
                        },
                    );
                    true
                }
                Err(e) => {
                    crate::log::error(format!(
                        "NXM install failed mod={} file={}: {e}",
                        link.mod_id, link.file_id
                    ));
                    let _ = app_handle.emit(
                        "nxm-download",
                        NxmProgressEvent {
                            stage: "error".into(),
                            mod_id: link.mod_id,
                            file_id: link.file_id,
                            message: e.to_string(),
                            folder_name: None,
                        },
                    );
                    false
                }
            };

            nxm_release(&dedupe, succeeded);
        });
    }
}

#[tauri::command]
pub fn nexus_mod_url(mod_id: u32) -> String {
    nexus_mod_page_url(mod_id)
}

/// File-specific Nexus page for free-user “Download through Nexus”.
#[tauri::command]
pub fn nexus_file_url(mod_id: u32, file_id: u32) -> String {
    nexus_file_page_url(mod_id, file_id)
}

#[tauri::command]
pub fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "BladeAndModManagerSourcery",
        "version": env!("CARGO_PKG_VERSION"),
        "game": "Blade & Sorcery (PCVR)",
        "nexusDomain": "bladeandsorcery",
        "steamAppId": "629730"
    })
}
