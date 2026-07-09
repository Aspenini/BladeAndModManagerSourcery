use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DATA_DIR_NAME: &str = "BladeAndSorcery_Data";
pub const EXE_NAME: &str = "BladeAndSorcery.exe";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePaths {
    pub game_root: String,
    pub data_dir: String,
    pub streaming_assets: String,
    pub mods_dir: String,
    pub exe_path: Option<String>,
}

/// Convert a path to a clean display/storage string.
/// On Windows, `canonicalize()` yields `\\?\D:\...` extended paths; strip that for UI/config.
pub fn path_to_string(path: &Path) -> String {
    normalize_path_string(&path.to_string_lossy())
}

/// Strip Windows extended-length prefixes (`\\?\`, `\\?\UNC\`) from a path string.
pub fn normalize_path_string(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

pub fn resolve_paths(game_root: &Path) -> AppResult<GamePaths> {
    let game_root = game_root
        .canonicalize()
        .unwrap_or_else(|_| game_root.to_path_buf());

    let data_dir = game_root.join(DATA_DIR_NAME);
    if !data_dir.is_dir() {
        return Err(AppError::msg(format!(
            "Not a valid Blade & Sorcery install: missing `{DATA_DIR_NAME}` under {}",
            path_to_string(&game_root)
        )));
    }

    let streaming_assets = data_dir.join("StreamingAssets");
    let mods_dir = streaming_assets.join("Mods");
    let exe = game_root.join(EXE_NAME);

    Ok(GamePaths {
        game_root: path_to_string(&game_root),
        data_dir: path_to_string(&data_dir),
        streaming_assets: path_to_string(&streaming_assets),
        mods_dir: path_to_string(&mods_dir),
        exe_path: if exe.exists() {
            Some(path_to_string(&exe))
        } else {
            None
        },
    })
}

pub fn validate_game_path(path: &Path) -> AppResult<GamePaths> {
    if !path.exists() {
        return Err(AppError::msg(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }
    resolve_paths(path)
}

pub fn ensure_mods_dir(game_root: &Path) -> AppResult<PathBuf> {
    let paths = resolve_paths(game_root)?;
    let mods = PathBuf::from(&paths.mods_dir);
    if !mods.exists() {
        // StreamingAssets may not exist until the game is launched once
        if let Some(parent) = mods.parent() {
            if !parent.exists() {
                return Err(AppError::msg(
                    "StreamingAssets folder is missing. Launch Blade & Sorcery once so the game creates it, then try again.",
                ));
            }
        }
        fs::create_dir_all(&mods)?;
    }
    Ok(mods)
}

pub fn is_disabled_folder_name(name: &str) -> bool {
    name.ends_with(".disabled")
}

pub fn enabled_name(name: &str) -> String {
    name.strip_suffix(".disabled").unwrap_or(name).to_string()
}

pub fn disabled_name(name: &str) -> String {
    if name.ends_with(".disabled") {
        name.to_string()
    } else {
        format!("{name}.disabled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_extended_path_prefix() {
        assert_eq!(
            normalize_path_string(r"\\?\D:\SteamLibrary\steamapps\common\Blade & Sorcery"),
            r"D:\SteamLibrary\steamapps\common\Blade & Sorcery"
        );
        assert_eq!(
            normalize_path_string(r"\\?\UNC\server\share\mods"),
            r"\\server\share\mods"
        );
        assert_eq!(
            normalize_path_string(r"D:\Games\BladeAndSorcery"),
            r"D:\Games\BladeAndSorcery"
        );
    }
}
