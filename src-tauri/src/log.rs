//! File logging next to config.json:
//! `%AppData%\BladeAndModManagerSourcery\logs\app-YYYY-MM-DD.log`

use crate::config::app_data_dir;
use crate::error::{AppError, AppResult};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn logs_dir() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("logs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn log_file_path() -> AppResult<PathBuf> {
    let day = Local::now().format("%Y-%m-%d");
    Ok(logs_dir()?.join(format!("app-{day}.log")))
}

fn write_line(level: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{timestamp}] [{level}] {message}\n");

    // Always mirror to stderr for `tauri dev` / console builds.
    eprint!("{line}");

    let Ok(path) = log_file_path() else {
        return;
    };
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

pub fn info(message: impl AsRef<str>) {
    write_line("INFO", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write_line("WARN", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write_line("ERROR", message.as_ref());
}

pub fn debug(message: impl AsRef<str>) {
    write_line("DEBUG", message.as_ref());
}

/// Wrap an IO error with path context for logs + UI.
pub fn io_ctx(path: impl AsRef<std::path::Path>, action: &str, err: std::io::Error) -> AppError {
    let path = path.as_ref().display();
    let msg = format!("IO error while {action} “{path}”: {err}");
    error(&msg);
    AppError::msg(msg)
}
