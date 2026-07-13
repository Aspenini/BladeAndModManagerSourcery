pub mod boxes;
pub mod install;
pub mod scan;

/// The game loads any folder under `StreamingAssets\Mods` whose root contains
/// this file — folder names are irrelevant to the loader. Disabling a mod
/// therefore works by renaming the manifest, never the folder.
pub const MANIFEST: &str = "manifest.json";
pub const MANIFEST_DISABLED: &str = "manifest.json.disabled";
