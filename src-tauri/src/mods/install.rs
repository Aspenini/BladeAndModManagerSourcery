use crate::config::{remove_installed, upsert_installed, InstalledRecord};
use crate::error::{AppError, AppResult};
use crate::game::paths::{disabled_name, enabled_name, ensure_mods_dir, is_disabled_folder_name};
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
        let disabled = mods_dir.join(disabled_name(&folder_name));
        if dest.exists() || disabled.exists() {
            // Idempotent success when this exact folder already exists from a
            // concurrent/duplicate NXM delivery of the same install.
            crate::log::warn(format!(
                "Mod folder already present ({folder_name}); treating install as complete"
            ));
            let _ = upsert_installed(InstalledRecord {
                folder_name: folder_name.clone(),
                nexus_mod_id,
                nexus_file_id,
                mod_name: mod_name.map(|s| s.to_string()),
                version: version.map(|s| s.to_string()),
                installed_at: Some(Utc::now().to_rfc3339()),
            });
            return Ok(folder_name);
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
            return Ok(());
        }
        return Err(AppError::msg(format!(
            "Mod folder not found: {folder_name}"
        )));
    }
    fs::remove_dir_all(&path)?;
    remove_installed(&enabled_name(folder_name))?;
    remove_installed(folder_name)?;
    Ok(())
}

pub fn set_mod_enabled(game_root: &Path, folder_name: &str, enabled: bool) -> AppResult<String> {
    validate_mod_folder_name(folder_name)?;
    let mods_dir = ensure_mods_dir(game_root)?;
    let current = mods_dir.join(folder_name);
    if !current.exists() {
        return Err(AppError::msg(format!(
            "Mod folder not found: {folder_name}"
        )));
    }

    let is_disabled = is_disabled_folder_name(folder_name);
    if enabled && is_disabled {
        let new_name = enabled_name(folder_name);
        let dest = mods_dir.join(&new_name);
        if dest.exists() {
            return Err(AppError::msg(format!(
                "Cannot enable: target folder already exists ({new_name})"
            )));
        }
        fs::rename(&current, &dest)?;
        return Ok(new_name);
    }
    if !enabled && !is_disabled {
        let new_name = disabled_name(folder_name);
        let dest = mods_dir.join(&new_name);
        if dest.exists() {
            return Err(AppError::msg(format!(
                "Cannot disable: target folder already exists ({new_name})"
            )));
        }
        fs::rename(&current, &dest)?;
        return Ok(new_name);
    }

    Ok(folder_name.to_string())
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
}
