mod commands;
mod config;
mod error;
mod game;
mod log;
mod mods;
mod nexus;
mod secret;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Single instance must be registered first so deep links from a second
    // process are forwarded to the running app instead of launching a duplicate.
    // With the deep-link feature, the primary instance already receives
    // on_open_url — do not also parse argv here or installs run twice.
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            crate::log::info("Second instance launched; focusing main window");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            if let Ok(dir) = crate::config::app_data_dir() {
                crate::log::info(format!("App data directory: {}", dir.display()));
            }
            if let Ok(dir) = crate::log::logs_dir() {
                crate::log::info(format!("Logging to: {}", dir.display()));
            }

            // Register nxm:// for installed apps and for `tauri dev` on Windows/Linux.
            #[cfg(any(windows, target_os = "linux"))]
            {
                match app.deep_link().register_all() {
                    Ok(()) => crate::log::info("Registered nxm:// protocol handler"),
                    Err(e) => crate::log::error(format!("Failed to register NXM deep link: {e}")),
                }
            }

            // Cold start: app opened via nxm:// while not running.
            // Dedup inside handle_nxm_urls also covers platforms that fire
            // on_open_url for the same URL.
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                let strings: Vec<String> = urls.into_iter().map(|u| u.to_string()).collect();
                if !strings.is_empty() {
                    crate::log::info(format!(
                        "Cold-start deep link(s): {}",
                        strings.join(" | ")
                    ));
                    commands::handle_nxm_urls(app.handle(), strings);
                }
            }

            // Warm: app already running receives another deep link.
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                let strings: Vec<String> = event.urls().into_iter().map(|u| u.to_string()).collect();
                crate::log::info(format!("Deep link event: {}", strings.join(" | ")));
                commands::handle_nxm_urls(&handle, strings);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_app_config,
            commands::detect_game_install,
            commands::validate_and_inspect_path,
            commands::confirm_game_path,
            commands::get_game_paths,
            commands::list_mods,
            commands::toggle_mod,
            commands::remove_mod,
            commands::import_mod_archive,
            commands::nexus_validate,
            commands::nexus_save_api_key,
            commands::nexus_clear_api_key,
            commands::nexus_get_user,
            commands::nexus_list_mods,
            commands::nexus_mod_detail,
            commands::nexus_mod_files,
            commands::nexus_download_and_install,
            commands::nexus_download_with_nxm,
            commands::nexus_mod_url,
            commands::nexus_file_url,
            commands::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
