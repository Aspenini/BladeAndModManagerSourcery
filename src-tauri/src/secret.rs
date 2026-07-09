//! Secure storage for the Nexus API key via the OS credential store
//! (Windows Credential Manager). Never written to config.json.

use crate::error::{AppError, AppResult};
use keyring::Entry;

const SERVICE: &str = "BladeAndModManagerSourcery";
const ACCOUNT: &str = "nexus-api-key";

fn entry() -> AppResult<Entry> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| {
        AppError::msg(format!("Could not open OS credential store: {e}"))
    })
}

/// Store the API key in the OS credential manager.
pub fn store_nexus_api_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::msg("API key is empty"));
    }
    entry()?
        .set_password(key)
        .map_err(|e| AppError::msg(format!("Failed to store API key securely: {e}")))
}

/// Load the API key from the OS credential manager.
pub fn load_nexus_api_key() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(password) => {
            let trimmed = password.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::msg(format!(
            "Failed to read API key from credential store: {e}"
        ))),
    }
}

/// Remove the API key from the OS credential manager (no-op if missing).
pub fn clear_nexus_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::msg(format!(
            "Failed to remove API key from credential store: {e}"
        ))),
    }
}

pub fn has_nexus_api_key() -> bool {
    matches!(load_nexus_api_key(), Ok(Some(_)))
}
