//! Parse Nexus `nxm://` protocol links used by "Mod Manager Download".
//!
//! Format:
//! `nxm://{game}/mods/{mod_id}/files/{file_id}?key=...&expires=...&user_id=...`

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Temporary free-user download ticket from an NXM link.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxmLink {
    pub game_domain: String,
    pub mod_id: u32,
    pub file_id: u32,
    pub key: String,
    pub expires: u64,
    pub user_id: Option<u64>,
    pub raw: String,
}

impl NxmLink {
    /// Whether the temporary key is still valid (with a small grace margin).
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Allow a few seconds of clock skew.
        self.expires + 5 < now
    }
}

/// Parse an `nxm://` URL into structured fields.
pub fn parse_nxm_url(url: &str) -> AppResult<NxmLink> {
    let raw = url.trim().to_string();
    let lower = raw.to_ascii_lowercase();
    if !lower.starts_with("nxm://") {
        return Err(AppError::msg(format!(
            "Not an NXM link (expected nxm://…): {raw}"
        )));
    }

    // Strip scheme without lowercasing the rest (key is case-sensitive).
    let rest = &raw[6..]; // after "nxm://"

    // Split path and query
    let (path_part, query_part) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };

    // path: {game}/mods/{mod_id}/files/{file_id}
    let segments: Vec<&str> = path_part
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() < 5
        || !segments[1].eq_ignore_ascii_case("mods")
        || !segments[3].eq_ignore_ascii_case("files")
    {
        return Err(AppError::msg(format!(
            "Unrecognized NXM path (expected game/mods/{{id}}/files/{{id}}): {raw}"
        )));
    }

    let game_domain = segments[0].to_string();
    let mod_id: u32 = segments[2]
        .parse()
        .map_err(|_| AppError::msg(format!("Invalid mod id in NXM link: {}", segments[2])))?;
    let file_id: u32 = segments[4]
        .parse()
        .map_err(|_| AppError::msg(format!("Invalid file id in NXM link: {}", segments[4])))?;

    let query = query_part.unwrap_or("");
    let mut key: Option<String> = None;
    let mut expires: Option<u64> = None;
    let mut user_id: Option<u64> = None;

    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((a, b)) => (a, b),
            None => (pair, ""),
        };
        match k.to_ascii_lowercase().as_str() {
            "key" => key = Some(percent_decode(v)),
            "expires" => {
                expires = Some(
                    v.parse()
                        .map_err(|_| AppError::msg(format!("Invalid expires value: {v}")))?,
                );
            }
            "user_id" => {
                if let Ok(n) = v.parse() {
                    user_id = Some(n);
                }
            }
            _ => {}
        }
    }

    let key = key.ok_or_else(|| {
        AppError::msg(
            "NXM link is missing the temporary download key. Choose Mod Manager Download → Slow Download on Nexus.",
        )
    })?;
    let expires = expires.ok_or_else(|| {
        AppError::msg("NXM link is missing the expires parameter.")
    })?;

    Ok(NxmLink {
        game_domain,
        mod_id,
        file_id,
        key,
        expires,
        user_id,
        raw,
    })
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h1 = from_hex(bytes[i + 1]);
                let h2 = from_hex(bytes[i + 2]);
                if let (Some(a), Some(b)) = (h1, h2) {
                    out.push((a << 4) | b);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_nxm() {
        let link = parse_nxm_url(
            "nxm://bladeandsorcery/mods/123/files/456?key=AbC_12%2B%3D&expires=1700000000&user_id=99",
        )
        .unwrap();
        assert_eq!(link.game_domain, "bladeandsorcery");
        assert_eq!(link.mod_id, 123);
        assert_eq!(link.file_id, 456);
        assert_eq!(link.key, "AbC_12+=");
        assert_eq!(link.expires, 1_700_000_000);
        assert_eq!(link.user_id, Some(99));
    }

    #[test]
    fn rejects_non_nxm() {
        assert!(parse_nxm_url("https://nexusmods.com/foo").is_err());
    }
}
