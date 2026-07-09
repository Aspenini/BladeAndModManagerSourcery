use crate::config::{load_installed, InstalledRecord};
use crate::error::AppResult;
use crate::game::paths::{
    enabled_name, ensure_mods_dir, is_disabled_folder_name, path_to_string,
};
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
    pub enabled: bool,
    pub path: String,
    pub has_manifest: bool,
    pub nexus_mod_id: Option<u32>,
    pub nexus_file_id: Option<u32>,
}

pub fn list_installed_mods(game_root: &Path) -> AppResult<Vec<LocalMod>> {
    let mods_dir = ensure_mods_dir(game_root)?;
    let index = load_installed().unwrap_or_default();
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

        let enabled = !is_disabled_folder_name(&folder_name);
        let logical_name = enabled_name(&folder_name);
        let manifest_path = path.join("manifest.json");
        let has_manifest = manifest_path.exists();
        let (display_name, author, version, description) = if has_manifest {
            parse_manifest(&manifest_path)
        } else {
            (logical_name.clone(), None, None, None)
        };

        let record = find_record(&index.mods, &folder_name, &logical_name);

        mods.push(LocalMod {
            folder_name: folder_name.clone(),
            display_name,
            author,
            version: version.or_else(|| record.and_then(|r| r.version.clone())),
            description,
            enabled,
            path: path_to_string(&path),
            has_manifest,
            nexus_mod_id: record.and_then(|r| r.nexus_mod_id),
            nexus_file_id: record.and_then(|r| r.nexus_file_id),
        });
    }

    mods.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(mods)
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

fn parse_manifest(path: &Path) -> (String, Option<String>, Option<String>, Option<String>) {
    let fallback = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| enabled_name(&n.to_string_lossy()))
        .unwrap_or_else(|| "Unknown mod".into());

    let Ok(data) = fs::read_to_string(path) else {
        return (fallback, None, None, None);
    };
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return (fallback, None, None, None);
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
    let version = first_str(&value, &["Version", "version", "ModVersion", "modVersion"]);
    let description = first_str(&value, &["Description", "description", "Desc", "desc"]);

    (name, author, version, description)
}

fn first_str(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(*key).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    None
}
