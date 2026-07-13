use crate::config::{load_boxes, load_installed, InstalledRecord};
use crate::error::AppResult;
use crate::game::paths::{
    enabled_name, ensure_mods_dir, is_disabled_folder_name, path_to_string,
};
use crate::mods::install::migrate_legacy_disabled_folder;
use crate::mods::{MANIFEST, MANIFEST_DISABLED};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalMod {
    pub folder_name: String,
    pub display_name: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    /// Game version this mod declares compatibility with (manifest GameVersion).
    pub game_version: Option<String>,
    pub enabled: bool,
    pub path: String,
    /// True when the folder has a manifest (live or parked) at its root —
    /// the game ignores folders without one, and only these can be toggled.
    pub has_manifest: bool,
    pub nexus_mod_id: Option<u32>,
    pub nexus_file_id: Option<u32>,
    /// Box (local collection) this mod belongs to, if any.
    pub box_id: Option<String>,
}

pub fn list_installed_mods(game_root: &Path) -> AppResult<Vec<LocalMod>> {
    let mods_dir = ensure_mods_dir(game_root)?;
    migrate_legacy_disabled_folders(&mods_dir);

    let index = load_installed().unwrap_or_default();
    let boxes = load_boxes().unwrap_or_default();
    let mut mods = Vec::new();

    let entries = match fs::read_dir(&mods_dir) {
        Ok(e) => e,
        Err(_) => return Ok(mods),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        if folder_name.starts_with('.') {
            continue;
        }

        let manifest_path = path.join(MANIFEST);
        let parked_path = path.join(MANIFEST_DISABLED);
        let enabled = manifest_path.exists();
        let has_manifest = enabled || parked_path.exists();

        let (display_name, author, version, description, game_version) = if enabled {
            parse_manifest(&manifest_path)
        } else if has_manifest {
            parse_manifest(&parked_path)
        } else {
            (folder_name.clone(), None, None, None, None)
        };

        let logical_name = enabled_name(&folder_name);
        let record = find_record(&index.mods, &folder_name, &logical_name);
        let box_id = crate::mods::boxes::box_id_for(&boxes, &folder_name);

        mods.push(LocalMod {
            folder_name: folder_name.clone(),
            display_name,
            author,
            version: version.or_else(|| record.and_then(|r| r.version.clone())),
            description,
            game_version,
            enabled,
            path: path_to_string(&path),
            has_manifest,
            nexus_mod_id: record.and_then(|r| r.nexus_mod_id),
            nexus_file_id: record.and_then(|r| r.nexus_file_id),
            box_id,
        });
    }

    mods.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(mods)
}

/// Self-healing pass: fold any `<Name>.disabled` folders (created by the old,
/// broken disable mechanism — the game loaded them anyway) into the
/// manifest-rename scheme.
fn migrate_legacy_disabled_folders(mods_dir: &Path) {
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_disabled_folder_name(&name) {
            migrate_legacy_disabled_folder(mods_dir, &name);
        }
    }
}

fn find_record<'a>(
    records: &'a [InstalledRecord],
    folder_name: &str,
    logical_name: &str,
) -> Option<&'a InstalledRecord> {
    records.iter().find(|r| {
        r.folder_name.eq_ignore_ascii_case(folder_name)
            || r.folder_name.eq_ignore_ascii_case(logical_name)
    })
}

type ManifestInfo = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn parse_manifest(path: &Path) -> ManifestInfo {
    let fallback = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| enabled_name(&n.to_string_lossy()))
        .unwrap_or_else(|| "Unknown mod".into());

    let Ok(data) = fs::read_to_string(path) else {
        return (fallback, None, None, None, None);
    };
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return (fallback, None, None, None, None);
    };

    let name = first_str(
        &value,
        &[
            "Name",
            "name",
            "ModName",
            "modName",
            "DisplayName",
            "displayName",
        ],
    )
    .unwrap_or(fallback);
    let author = first_str(&value, &["Author", "author", "Creator", "creator"]);
    let version = first_str(&value, &["ModVersion", "modVersion", "Version", "version"]);
    let description = first_str(&value, &["Description", "description", "Desc", "desc"]);
    let game_version = first_str(&value, &["GameVersion", "gameVersion"]);

    (name, author, version, description, game_version)
}

fn first_str(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(*key) {
            // GameVersion is sometimes a bare number in older manifests.
            let text = match s {
                Value::String(s) => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}
