//! Mod "boxes" — named local collections (e.g. per game version like "U7"
//! or "1.0"). A mod can belong to at most one box. Activating a box enables
//! its mods and disables every mod that belongs to a *different* box; mods
//! outside any box are left untouched.

use crate::config::{load_boxes, save_boxes, BoxesFile, ModBox};
use crate::error::{AppError, AppResult};
use crate::mods::install::set_mod_enabled;
use crate::mods::scan::list_installed_mods;
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateReport {
    pub box_id: String,
    pub box_name: String,
    pub enabled: usize,
    pub disabled: usize,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

fn new_box_id() -> String {
    format!("box-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default())
}

fn validate_name(state: &BoxesFile, name: &str, ignore_id: Option<&str>) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::msg("Box name cannot be empty."));
    }
    if trimmed.chars().count() > 64 {
        return Err(AppError::msg("Box name is too long (max 64 characters)."));
    }
    let clash = state.boxes.iter().any(|b| {
        b.name.eq_ignore_ascii_case(trimmed) && ignore_id.map_or(true, |id| b.id != id)
    });
    if clash {
        return Err(AppError::msg(format!(
            "A box named “{trimmed}” already exists."
        )));
    }
    Ok(trimmed.to_string())
}

pub fn get_boxes() -> AppResult<BoxesFile> {
    load_boxes()
}

pub fn create_box(name: &str) -> AppResult<BoxesFile> {
    let mut state = load_boxes()?;
    let name = validate_name(&state, name, None)?;
    state.boxes.push(ModBox {
        id: new_box_id(),
        name,
        created_at: Some(Utc::now().to_rfc3339()),
    });
    save_boxes(&state)?;
    Ok(state)
}

pub fn rename_box(box_id: &str, name: &str) -> AppResult<BoxesFile> {
    let mut state = load_boxes()?;
    let name = validate_name(&state, name, Some(box_id))?;
    let entry = state
        .boxes
        .iter_mut()
        .find(|b| b.id == box_id)
        .ok_or_else(|| AppError::msg("Box not found."))?;
    entry.name = name;
    save_boxes(&state)?;
    Ok(state)
}

/// Delete a box. Its mods are kept installed — they simply become unboxed.
pub fn delete_box(box_id: &str) -> AppResult<BoxesFile> {
    let mut state = load_boxes()?;
    let before = state.boxes.len();
    state.boxes.retain(|b| b.id != box_id);
    if state.boxes.len() == before {
        return Err(AppError::msg("Box not found."));
    }
    state.assignments.retain(|_, v| v != box_id);
    if state.active_box_id.as_deref() == Some(box_id) {
        state.active_box_id = None;
    }
    save_boxes(&state)?;
    Ok(state)
}

/// Assign a mod to a box, or remove it from any box when `box_id` is None.
pub fn assign_mod(folder_name: &str, box_id: Option<&str>) -> AppResult<BoxesFile> {
    let mut state = load_boxes()?;
    // Normalize the key so lookups stay case-stable on Windows.
    let existing_key = state
        .assignments
        .keys()
        .find(|k| k.eq_ignore_ascii_case(folder_name))
        .cloned();
    if let Some(key) = existing_key {
        state.assignments.remove(&key);
    }
    if let Some(id) = box_id {
        if !state.boxes.iter().any(|b| b.id == id) {
            return Err(AppError::msg("Box not found."));
        }
        state
            .assignments
            .insert(folder_name.to_string(), id.to_string());
    }
    save_boxes(&state)?;
    Ok(state)
}

/// Look up which box (if any) a folder belongs to.
pub fn box_id_for(state: &BoxesFile, folder_name: &str) -> Option<String> {
    state
        .assignments
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(folder_name))
        .map(|(_, v)| v.clone())
}

/// Called after a successful install: new mods join the active box, if any.
pub fn assign_to_active_box(folder_name: &str) {
    let Ok(state) = load_boxes() else { return };
    let Some(active) = state.active_box_id.clone() else {
        return;
    };
    if let Err(e) = assign_mod(folder_name, Some(&active)) {
        crate::log::warn(format!(
            "Could not assign {folder_name} to active box: {e}"
        ));
    }
}

/// Enable every mod in the box, disable mods that belong to other boxes,
/// leave unboxed mods alone. Also prunes assignments to vanished folders.
pub fn activate_box(game_root: &Path, box_id: &str) -> AppResult<ActivateReport> {
    let mut state = load_boxes()?;
    let box_name = state
        .boxes
        .iter()
        .find(|b| b.id == box_id)
        .map(|b| b.name.clone())
        .ok_or_else(|| AppError::msg("Box not found."))?;

    let mods = list_installed_mods(game_root)?;

    // Prune assignments pointing at folders that no longer exist.
    state.assignments.retain(|folder, _| {
        mods.iter().any(|m| m.folder_name.eq_ignore_ascii_case(folder))
    });

    let mut report = ActivateReport {
        box_id: box_id.to_string(),
        box_name,
        enabled: 0,
        disabled: 0,
        skipped: Vec::new(),
        errors: Vec::new(),
    };

    for m in &mods {
        let assigned = box_id_for(&state, &m.folder_name);
        let want_enabled = match assigned.as_deref() {
            Some(id) if id == box_id => true,
            Some(_) => false,
            None => continue, // unboxed mods are not touched
        };
        if m.enabled == want_enabled {
            continue;
        }
        if !m.has_manifest {
            report.skipped.push(m.folder_name.clone());
            continue;
        }
        match set_mod_enabled(game_root, &m.folder_name, want_enabled) {
            Ok(_) => {
                if want_enabled {
                    report.enabled += 1;
                } else {
                    report.disabled += 1;
                }
            }
            Err(e) => report
                .errors
                .push(format!("{}: {}", m.folder_name, e)),
        }
    }

    state.active_box_id = Some(box_id.to_string());
    save_boxes(&state)?;
    crate::log::info(format!(
        "Activated box “{}”: +{} enabled, {} disabled, {} skipped, {} errors",
        report.box_name,
        report.enabled,
        report.disabled,
        report.skipped.len(),
        report.errors.len()
    ));
    Ok(report)
}

/// Clear the active box without touching any mod state.
pub fn clear_active_box() -> AppResult<BoxesFile> {
    let mut state = load_boxes()?;
    state.active_box_id = None;
    save_boxes(&state)?;
    Ok(state)
}
