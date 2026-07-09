use crate::error::AppResult;
use crate::game::paths::{validate_game_path, DATA_DIR_NAME, EXE_NAME};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Steam AppID for Blade & Sorcery (PCVR).
pub const STEAM_APP_ID: &str = "629730";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGame {
    pub path: String,
    pub source: String,
    pub valid: bool,
    pub message: String,
}

pub fn detect_game() -> AppResult<Option<DetectedGame>> {
    if let Some(path) = find_steam_install() {
        return Ok(Some(build_detection(path, "steam")?));
    }
    if let Some(path) = find_oculus_install() {
        return Ok(Some(build_detection(path, "oculus")?));
    }
    Ok(None)
}

fn build_detection(path: PathBuf, source: &str) -> AppResult<DetectedGame> {
    match validate_game_path(&path) {
        Ok(paths) => Ok(DetectedGame {
            path: paths.game_root,
            source: source.to_string(),
            valid: true,
            message: "Blade & Sorcery install found.".into(),
        }),
        Err(e) => Ok(DetectedGame {
            path: path.to_string_lossy().to_string(),
            source: source.to_string(),
            valid: false,
            message: e.to_string(),
        }),
    }
}

pub fn inspect_path(path: &Path) -> AppResult<DetectedGame> {
    let paths = validate_game_path(path)?;
    Ok(DetectedGame {
        path: paths.game_root,
        source: "manual".into(),
        valid: true,
        message: "Valid Blade & Sorcery install.".into(),
    })
}

fn find_steam_install() -> Option<PathBuf> {
    let steam_root = steam_install_path()?;
    let libraries = steam_library_paths(&steam_root);

    for lib in libraries {
        let manifest = lib
            .join("steamapps")
            .join(format!("appmanifest_{STEAM_APP_ID}.acf"));
        if !manifest.exists() {
            // Some layouts put steamapps at the library root already
            let alt = lib.join(format!("appmanifest_{STEAM_APP_ID}.acf"));
            if alt.exists() {
                if let Some(dir) = install_dir_from_manifest(&alt) {
                    let root = lib.join("common").join(dir);
                    if looks_like_game(&root) {
                        return Some(root);
                    }
                }
            }
            continue;
        }

        if let Some(dir) = install_dir_from_manifest(&manifest) {
            let root = lib.join("steamapps").join("common").join(&dir);
            if looks_like_game(&root) {
                return Some(root);
            }
            // Fallback common folder name
            let fallback = lib.join("steamapps").join("common").join("Blade & Sorcery");
            if looks_like_game(&fallback) {
                return Some(fallback);
            }
        }
    }

    // Last resort: scan common folders for the game
    for lib in steam_library_paths(&steam_root) {
        let common = lib.join("steamapps").join("common");
        if let Ok(entries) = fs::read_dir(common) {
            for entry in entries.flatten() {
                let p = entry.path();
                if looks_like_game(&p) {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if name.contains("blade") && name.contains("sorcery") {
                        return Some(p);
                    }
                }
            }
        }
    }

    None
}

fn steam_install_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                let p = PathBuf::from(path.replace('/', "\\"));
                if p.exists() {
                    return Some(p);
                }
            }
        }

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for sub in [
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            "SOFTWARE\\Valve\\Steam",
        ] {
            if let Ok(key) = hklm.open_subkey(sub) {
                if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                    let p = PathBuf::from(path);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }

        let defaults = [
            r"C:\Program Files (x86)\Steam",
            r"C:\Program Files\Steam",
            r"D:\Steam",
            r"D:\SteamLibrary",
        ];
        for d in defaults {
            let p = PathBuf::from(d);
            if p.join("steam.exe").exists() || p.join("steamapps").exists() {
                return Some(p);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn steam_library_paths(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_root.to_path_buf()];
    let vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(&vdf) {
        for line in content.lines() {
            // Match "path"		"D:\\SteamLibrary"  or "1"  "D:\\..."
            let trimmed = line.trim();
            if let Some(path) = extract_vdf_path(trimmed) {
                let p = PathBuf::from(path.replace("\\\\", "\\"));
                if p.exists() && !libs.iter().any(|l| l == &p) {
                    libs.push(p);
                }
            }
        }
    }
    libs
}

fn extract_vdf_path(line: &str) -> Option<String> {
    // "path"		"C:\\Steam"
    let lower = line.to_lowercase();
    if lower.contains("\"path\"") {
        let parts: Vec<&str> = line.split('"').collect();
        // ["", "path", "\t\t", "C:\\Steam", ""]
        if parts.len() >= 4 {
            let candidate = parts[parts.len() - 2];
            if candidate.len() > 1 && (candidate.contains(':') || candidate.starts_with('/')) {
                return Some(candidate.to_string());
            }
        }
    }
    // Older format: "1"   "D:\\Games\\Steam"
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 {
        let key = parts[1];
        let val = parts[3];
        if key.chars().all(|c| c.is_ascii_digit())
            && val.len() > 2
            && (val.contains(':') || val.starts_with('/'))
            && !val.to_lowercase().contains("steamapps")
        {
            return Some(val.to_string());
        }
    }
    None
}

fn install_dir_from_manifest(manifest: &Path) -> Option<String> {
    let content = fs::read_to_string(manifest).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().contains("\"installdir\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                return Some(parts[3].to_string());
            }
        }
    }
    None
}

fn looks_like_game(path: &Path) -> bool {
    path.join(DATA_DIR_NAME).is_dir() || path.join(EXE_NAME).is_file()
}

fn find_oculus_install() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let candidates = [
            r"C:\Program Files\Oculus\Software\Software\warpfrog-blade-sorcery",
            r"C:\Program Files\Oculus\Software\warpfrog-blade-sorcery",
            r"D:\Oculus\Software\Software\warpfrog-blade-sorcery",
        ];
        for c in candidates {
            let p = PathBuf::from(c);
            if looks_like_game(&p) {
                return Some(p);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}
