use crate::config::{remove_installed, upsert_installed, InstalledRecord};
use crate::error::{AppError, AppResult};
use crate::game::paths::{disabled_name, enabled_name, ensure_mods_dir, is_disabled_folder_name};
use crate::mods::{MANIFEST, MANIFEST_DISABLED};
use chrono::Utc;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

const MAX_ARCHIVE_FILES: usize = 10_000;
const MAX_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn install_from_archive(
    game_root: &Path,
    archive_path: &Path,
    preferred_name: Option<&str>,
    nexus_mod_id: Option<u32>,
    nexus_file_id: Option<u32>,
    mod_name: Option<&str>,
    version: Option<&str>,
) -> AppResult<String> {
    crate::log::info(format!(
        "Install start: archive={} game={} preferred={:?} mod={:?} ver={:?}",
        archive_path.display(),
        game_root.display(),
        preferred_name,
        mod_name,
        version
    ));

    let mods_dir = ensure_mods_dir(game_root)?;
    if !archive_path.is_file() {
        let msg = format!(
            "The selected archive is not a readable file: {}",
            archive_path.display()
        );
        crate::log::error(&msg);
        return Err(AppError::msg(msg));
    }

    // Use app-owned staging space rather than the archive's directory. Imported
    // files may live in a protected/read-only folder, and this also keeps cleanup
    // predictable.
    let extract_root = crate::config::app_data_dir()?
        .join("install-staging")
        .join(format!(
            "{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
    fs::create_dir_all(&extract_root).map_err(|e| {
        crate::log::io_ctx(&extract_root, "creating install staging directory", e)
    })?;

    let result = (|| {
        extract_zip(archive_path, &extract_root)?;

        let mod_root = find_mod_root(&extract_root)?;
        crate::log::debug(format!("Resolved mod root: {}", mod_root.display()));

        let folder_name_owned = preferred_name
            .or(mod_name)
            .map(|s| s.to_string())
            .or_else(|| {
                mod_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "InstalledMod".into());
        let folder_name = sanitize_folder_name(&folder_name_owned);

        let dest = mods_dir.join(&folder_name);
        // Replace an existing copy so installing again acts as update/reinstall.
        // (Concurrent duplicate NXM deliveries are already deduped upstream.)
        for existing in [dest.clone(), mods_dir.join(disabled_name(&folder_name))] {
            if existing.exists() {
                crate::log::warn(format!(
                    "Replacing existing mod folder: {}",
                    existing.display()
                ));
                fs::remove_dir_all(&existing).map_err(|e| {
                    crate::log::io_ctx(&existing, "removing previous mod version", e)
                })?;
            }
        }

        if let Err(error) = copy_dir_recursive(&mod_root, &dest) {
            // Do not leave a partially copied mod behind if the disk fills up or
            // a file cannot be read during installation.
            let _ = fs::remove_dir_all(&dest);
            crate::log::error(format!("Copy into mods dir failed: {error}"));
            return Err(error);
        }

        upsert_installed(InstalledRecord {
            folder_name: folder_name.clone(),
            nexus_mod_id,
            nexus_file_id,
            mod_name: mod_name.map(|s| s.to_string()),
            version: version.map(|s| s.to_string()),
            installed_at: Some(Utc::now().to_rfc3339()),
        })?;

        // New installs join the active box, if one is selected.
        crate::mods::boxes::assign_to_active_box(&folder_name);

        crate::log::info(format!(
            "Install complete: folder={folder_name} dest={}",
            dest.display()
        ));
        Ok(folder_name)
    })();

    if let Err(ref e) = result {
        crate::log::error(format!("Install failed: {e}"));
    }
    let _ = fs::remove_dir_all(&extract_root);
    result
}

pub fn uninstall_mod(game_root: &Path, folder_name: &str) -> AppResult<()> {
    validate_mod_folder_name(folder_name)?;
    let mods_dir = ensure_mods_dir(game_root)?;
    let path = mods_dir.join(folder_name);
    if !path.exists() {
        // Try enabled/disabled variants
        let alt = if is_disabled_folder_name(folder_name) {
            mods_dir.join(enabled_name(folder_name))
        } else {
            mods_dir.join(disabled_name(folder_name))
        };
        if alt.exists() {
            fs::remove_dir_all(&alt)?;
            remove_installed(&enabled_name(folder_name))?;
            remove_installed(folder_name)?;
            let _ = crate::mods::boxes::assign_mod(folder_name, None);
            return Ok(());
        }
        return Err(AppError::msg(format!(
            "Mod folder not found: {folder_name}"
        )));
    }
    fs::remove_dir_all(&path)?;
    remove_installed(&enabled_name(folder_name))?;
    remove_installed(folder_name)?;
    let _ = crate::mods::boxes::assign_mod(folder_name, None);
    Ok(())
}

/// Enable or disable a mod in place by renaming its `manifest.json`.
///
/// The game loads every folder in `Mods` whose root contains a manifest —
/// folder names mean nothing to it, so renaming the folder (the old approach)
/// never actually disabled anything. Renaming the manifest makes the loader
/// skip the folder entirely, and the mod's files stay where they are.
pub fn set_mod_enabled(game_root: &Path, folder_name: &str, enabled: bool) -> AppResult<String> {
    validate_mod_folder_name(folder_name)?;
    let mods_dir = ensure_mods_dir(game_root)?;
    let dir = mods_dir.join(folder_name);
    if !dir.is_dir() {
        return Err(AppError::msg(format!(
            "Mod folder not found: {folder_name}"
        )));
    }

    let active = dir.join(MANIFEST);
    let dormant = dir.join(MANIFEST_DISABLED);

    if enabled {
        if active.exists() {
            return Ok(folder_name.to_string()); // already enabled
        }
        if !dormant.exists() {
            return Err(AppError::msg(format!(
                "Cannot enable “{folder_name}”: no manifest.json found in the mod folder."
            )));
        }
        fs::rename(&dormant, &active)
            .map_err(|e| crate::log::io_ctx(&dormant, "restoring manifest.json", e))?;
    } else {
        if dormant.exists() && !active.exists() {
            return Ok(folder_name.to_string()); // already disabled
        }
        if !active.exists() {
            return Err(AppError::msg(format!(
                "Cannot disable “{folder_name}”: no manifest.json found in the mod folder."
            )));
        }
        // If a stale manifest.json.disabled is in the way, drop it in favor of
        // the live manifest.
        if dormant.exists() {
            fs::remove_file(&dormant)
                .map_err(|e| crate::log::io_ctx(&dormant, "removing stale disabled manifest", e))?;
        }
        fs::rename(&active, &dormant)
            .map_err(|e| crate::log::io_ctx(&active, "parking manifest.json", e))?;
    }

    crate::log::info(format!(
        "Mod “{folder_name}” is now {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    Ok(folder_name.to_string())
}

/// Migrate a legacy `<Name>.disabled` folder (the old, non-functional disable
/// mechanism) into the manifest-rename scheme: restore the folder name and
/// park its manifest, preserving the user's intent that the mod be disabled.
pub fn migrate_legacy_disabled_folder(mods_dir: &Path, folder_name: &str) -> Option<String> {
    if !is_disabled_folder_name(folder_name) {
        return None;
    }
    let src = mods_dir.join(folder_name);
    let base = enabled_name(folder_name);
    let dest = mods_dir.join(&base);

    // Park the manifest first so the mod stops loading even if the folder
    // rename below cannot happen.
    let active = src.join(MANIFEST);
    let dormant = src.join(MANIFEST_DISABLED);
    if active.exists() && !dormant.exists() {
        let _ = fs::rename(&active, &dormant);
    }

    if dest.exists() {
        crate::log::warn(format!(
            "Legacy disabled folder “{folder_name}” conflicts with “{base}”; leaving name as-is"
        ));
        return None;
    }
    match fs::rename(&src, &dest) {
        Ok(()) => {
            crate::log::info(format!(
                "Migrated legacy disabled mod “{folder_name}” → “{base}” (manifest parked)"
            ));
            Some(base)
        }
        Err(e) => {
            crate::log::warn(format!(
                "Could not rename legacy disabled folder “{folder_name}”: {e}"
            ));
            None
        }
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> AppResult<()> {
    let file = fs::File::open(archive)
        .map_err(|e| crate::log::io_ctx(archive, "opening archive for extraction", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        let msg = format!("Invalid or unreadable ZIP {}: {e}", archive.display());
        crate::log::error(&msg);
        AppError::msg(msg)
    })?;

    if archive.len() > MAX_ARCHIVE_FILES {
        return Err(AppError::msg(format!(
            "Archive has too many files (maximum {MAX_ARCHIVE_FILES})."
        )));
    }

    let mut total_uncompressed = 0_u64;
    let mut extracted = 0usize;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            let msg = format!("Failed reading zip entry #{i}: {e}");
            crate::log::error(&msg);
            AppError::msg(msg)
        })?;
        total_uncompressed = total_uncompressed.saturating_add(file.size());
        if total_uncompressed > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
            return Err(AppError::msg(
                "Archive expands beyond the 2 GB safety limit and was not installed.",
            ));
        }
        let outpath = match file.enclosed_name() {
            Some(p) => dest.join(p),
            None => {
                crate::log::warn(format!("Skipping unsafe zip path: {}", file.name()));
                continue;
            }
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| crate::log::io_ctx(&outpath, "creating extracted directory", e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| crate::log::io_ctx(parent, "creating parent for extracted file", e))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| crate::log::io_ctx(&outpath, "creating extracted file", e))?;
            io::copy(&mut file, &mut outfile)
                .map_err(|e| crate::log::io_ctx(&outpath, "writing extracted file", e))?;
            extracted += 1;
        }
    }
    crate::log::info(format!("Extracted {extracted} file(s) from archive"));
    Ok(())
}

fn find_mod_root(extract_root: &Path) -> AppResult<PathBuf> {
    // Prefer directory containing manifest.json
    for entry in WalkDir::new(extract_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name().eq_ignore_ascii_case("manifest.json") {
            if let Some(parent) = entry.path().parent() {
                return Ok(parent.to_path_buf());
            }
        }
    }

    // If extract root has a single subdirectory, use that
    let mut children: Vec<PathBuf> = fs::read_dir(extract_root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    // Ignore macOS junk
    children.retain(|p| {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        name != "__MACOSX"
    });

    if children.len() == 1 {
        return Ok(children.remove(0));
    }

    // Otherwise install contents of extract root itself
    Ok(extract_root.to_path_buf())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> AppResult<()> {
    fs::create_dir_all(dest)
        .map_err(|e| crate::log::io_ctx(dest, "creating mod destination directory", e))?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| crate::log::io_ctx(&target, "creating mod subdirectory", e))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    crate::log::io_ctx(parent, "creating parent for mod file copy", e)
                })?;
            }
            fs::copy(entry.path(), &target).map_err(|e| {
                crate::log::io_ctx(
                    entry.path(),
                    &format!("copying into {}", target.display()),
                    e,
                )
            })?;
        }
    }
    Ok(())
}

fn sanitize_folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.');
    if cleaned.is_empty() {
        "InstalledMod".into()
    } else {
        cleaned.to_string()
    }
}

fn validate_mod_folder_name(name: &str) -> AppResult<()> {
    let path = Path::new(name);
    let is_single_normal_component = matches!(
        path.components().next(),
        Some(std::path::Component::Normal(_))
    ) && path.components().count() == 1;

    if !is_single_normal_component || name.trim().is_empty() {
        return Err(AppError::msg("Invalid mod folder name."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_the_mods_directory() {
        assert!(validate_mod_folder_name("..\\outside").is_err());
        assert!(validate_mod_folder_name("nested/mod").is_err());
        assert!(validate_mod_folder_name(".").is_err());
        assert!(validate_mod_folder_name("GoodMod.disabled").is_ok());
    }

    #[test]
    fn sanitizes_windows_invalid_characters() {
        assert_eq!(sanitize_folder_name("A/B: C"), "A_B_ C");
    }

    fn temp_game_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "bams-test-{tag}-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let mods = root.join("BladeAndSorcery_Data/StreamingAssets/Mods");
        fs::create_dir_all(&mods).unwrap();
        root
    }

    fn mods_dir_of(root: &Path) -> PathBuf {
        root.join("BladeAndSorcery_Data/StreamingAssets/Mods")
    }

    #[test]
    fn disable_parks_the_manifest_and_keeps_the_folder_name() {
        let root = temp_game_root("toggle");
        let mod_dir = mods_dir_of(&root).join("CoolSword");
        fs::create_dir_all(&mod_dir).unwrap();
        fs::write(mod_dir.join(MANIFEST), "{\"Name\":\"Cool Sword\"}").unwrap();

        let name = set_mod_enabled(&root, "CoolSword", false).unwrap();
        assert_eq!(name, "CoolSword");
        assert!(!mod_dir.join(MANIFEST).exists());
        assert!(mod_dir.join(MANIFEST_DISABLED).exists());

        // Re-enable restores the manifest.
        set_mod_enabled(&root, "CoolSword", true).unwrap();
        assert!(mod_dir.join(MANIFEST).exists());
        assert!(!mod_dir.join(MANIFEST_DISABLED).exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn toggle_is_idempotent_and_errors_without_manifest() {
        let root = temp_game_root("idem");
        let mod_dir = mods_dir_of(&root).join("NoManifest");
        fs::create_dir_all(&mod_dir).unwrap();

        assert!(set_mod_enabled(&root, "NoManifest", false).is_err());
        assert!(set_mod_enabled(&root, "NoManifest", true).is_err());

        fs::write(mod_dir.join(MANIFEST), "{}").unwrap();
        set_mod_enabled(&root, "NoManifest", true).unwrap(); // already enabled: no-op
        assert!(mod_dir.join(MANIFEST).exists());
        set_mod_enabled(&root, "NoManifest", false).unwrap();
        set_mod_enabled(&root, "NoManifest", false).unwrap(); // already disabled: no-op
        assert!(mod_dir.join(MANIFEST_DISABLED).exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn migrates_legacy_disabled_folder_to_parked_manifest() {
        let root = temp_game_root("legacy");
        let mods = mods_dir_of(&root);
        let legacy = mods.join("OldMod.disabled");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(MANIFEST), "{\"Name\":\"Old Mod\"}").unwrap();

        let migrated = migrate_legacy_disabled_folder(&mods, "OldMod.disabled");
        assert_eq!(migrated.as_deref(), Some("OldMod"));
        let new_dir = mods.join("OldMod");
        assert!(new_dir.is_dir());
        assert!(!legacy.exists());
        assert!(new_dir.join(MANIFEST_DISABLED).exists());
        assert!(!new_dir.join(MANIFEST).exists());

        let _ = fs::remove_dir_all(&root);
    }
}
