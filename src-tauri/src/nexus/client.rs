use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub const GAME_DOMAIN: &str = "bladeandsorcery";
const BASE_URL: &str = "https://api.nexusmods.com/v1";
const APP_NAME: &str = "BladeAndModManagerSourcery";
const APP_VERSION: &str = "0.1.0";

#[derive(Clone)]
pub struct NexusClient {
    api_key: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusUser {
    pub name: Option<String>,
    pub is_premium: bool,
    pub is_supporter: bool,
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusModSummary {
    pub mod_id: u32,
    pub name: String,
    pub summary: Option<String>,
    pub picture_url: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub endorsements: Option<u64>,
    pub downloads: Option<u64>,
    pub category_id: Option<u32>,
    pub created_time: Option<String>,
    pub updated_time: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusModDetail {
    pub mod_id: u32,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub picture_url: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub endorsements: Option<u64>,
    pub downloads: Option<u64>,
    pub category_id: Option<u32>,
    pub created_time: Option<String>,
    pub updated_time: Option<String>,
    pub available: bool,
    pub contains_adult_content: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFile {
    pub file_id: u32,
    pub name: String,
    pub version: Option<String>,
    pub category_name: Option<String>,
    pub size_kb: Option<u64>,
    pub uploaded_time: Option<String>,
    pub description: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLink {
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub uri: String,
}

impl NexusClient {
    pub fn new(api_key: impl Into<String>) -> AppResult<Self> {
        let api_key = api_key.into().trim().to_string();
        if api_key.is_empty() {
            return Err(AppError::msg("Nexus API key is empty"));
        }
        let http = reqwest::Client::builder()
            .user_agent(format!("{APP_NAME}/{APP_VERSION}"))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { api_key, http })
    }

    fn headers(&self) -> AppResult<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        let api_key = self
            .api_key
            .parse()
            .map_err(|_| AppError::msg("Nexus API key contains invalid characters."))?;
        headers.insert("apikey", api_key);
        headers.insert(
            "Application-Name",
            APP_NAME.parse().expect("static header is valid"),
        );
        headers.insert(
            "Application-Version",
            APP_VERSION.parse().expect("static header is valid"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json".parse().expect("static header is valid"),
        );
        Ok(headers)
    }

    async fn get_json(&self, path: &str) -> AppResult<Value> {
        let url = format!("{BASE_URL}{path}");
        let res = self.http.get(&url).headers(self.headers()?).send().await?;
        let status = res.status();
        let body = res.text().await?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "Nexus API {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn validate_key(&self) -> AppResult<NexusUser> {
        let v = self.get_json("/users/validate.json").await?;
        Ok(NexusUser {
            name: v
                .get("name")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            is_premium: v
                .get("is_premium")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            is_supporter: v
                .get("is_supporter")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            profile_url: v
                .get("profile_url")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
        })
    }

    pub async fn trending(&self) -> AppResult<Vec<NexusModSummary>> {
        let v = self
            .get_json(&format!("/games/{GAME_DOMAIN}/mods/trending.json"))
            .await?;
        Ok(parse_mod_list(&v))
    }

    pub async fn latest_added(&self) -> AppResult<Vec<NexusModSummary>> {
        let v = self
            .get_json(&format!("/games/{GAME_DOMAIN}/mods/latest_added.json"))
            .await?;
        Ok(parse_mod_list(&v))
    }

    pub async fn latest_updated(&self) -> AppResult<Vec<NexusModSummary>> {
        let v = self
            .get_json(&format!("/games/{GAME_DOMAIN}/mods/latest_updated.json"))
            .await?;
        Ok(parse_mod_list(&v))
    }

    /// Free-text search via Nexus mods endpoint with terms.
    /// Falls back to filtering trending if search endpoint fails.
    pub async fn search(&self, terms: &str) -> AppResult<Vec<NexusModSummary>> {
        let terms = terms.trim();
        if terms.is_empty() {
            return self.trending().await;
        }

        // Nexus public API search (form of include_adult + terms)
        let encoded: Vec<String> = terms
            .split_whitespace()
            .map(|t| urlencoding_simple(t))
            .collect();
        let path = format!("/games/{GAME_DOMAIN}/mods.json?terms={}", encoded.join(","));

        match self.get_json(&path).await {
            Ok(v) => {
                let list = parse_mod_list(&v);
                if !list.is_empty() {
                    return Ok(list);
                }
            }
            Err(_) => {}
        }

        // Fallback: local filter over trending + latest
        let mut all = self.trending().await.unwrap_or_default();
        if let Ok(latest) = self.latest_added().await {
            for m in latest {
                if !all.iter().any(|x| x.mod_id == m.mod_id) {
                    all.push(m);
                }
            }
        }
        let q = terms.to_lowercase();
        all.retain(|m| {
            m.name.to_lowercase().contains(&q)
                || m.summary
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
                || m.author
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
        });
        Ok(all)
    }

    pub async fn mod_detail(&self, mod_id: u32) -> AppResult<NexusModDetail> {
        let v = self
            .get_json(&format!("/games/{GAME_DOMAIN}/mods/{mod_id}.json"))
            .await?;
        Ok(NexusModDetail {
            mod_id: v
                .get("mod_id")
                .and_then(|x| x.as_u64())
                .unwrap_or(mod_id as u64) as u32,
            name: v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            summary: v
                .get("summary")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            description: v
                .get("description")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            picture_url: v
                .get("picture_url")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            author: v
                .get("author")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            version: v
                .get("version")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            endorsements: v.get("endorsement_count").and_then(|x| x.as_u64()),
            downloads: v
                .get("mod_downloads")
                .and_then(|x| x.as_u64())
                .or_else(|| v.get("downloads").and_then(|x| x.as_u64())),
            category_id: v
                .get("category_id")
                .and_then(|x| x.as_u64())
                .map(|n| n as u32),
            created_time: v
                .get("created_time")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            updated_time: v
                .get("updated_time")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            available: v.get("available").and_then(|x| x.as_bool()).unwrap_or(true),
            contains_adult_content: v.get("contains_adult_content").and_then(|x| x.as_bool()),
        })
    }

    pub async fn list_files(&self, mod_id: u32) -> AppResult<Vec<NexusFile>> {
        let v = self
            .get_json(&format!("/games/{GAME_DOMAIN}/mods/{mod_id}/files.json"))
            .await?;

        let files = v
            .get("files")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out = Vec::new();
        for f in files {
            let category = f
                .get("category_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();

            let file_id = match f.get("file_id").and_then(|x| x.as_u64()) {
                Some(id) => id as u32,
                None => continue,
            };

            out.push(NexusFile {
                file_id,
                name: f
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("file")
                    .to_string(),
                version: f
                    .get("version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                category_name: if category.is_empty() {
                    None
                } else {
                    Some(category)
                },
                size_kb: f.get("size_kb").and_then(|x| x.as_u64()),
                uploaded_time: f
                    .get("uploaded_time")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                description: f
                    .get("description")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                is_primary: f
                    .get("is_primary")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false)
                    || f.get("category_name")
                        .and_then(|x| x.as_str())
                        .map(|s| s.eq_ignore_ascii_case("MAIN"))
                        .unwrap_or(false),
            });
        }

        // Prefer primary / main first
        out.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));
        Ok(out)
    }

    /// Resolve CDN download mirrors.
    ///
    /// Premium accounts can call this without a ticket. Free accounts must supply the temporary
    /// `key` + `expires` from an `nxm://` link (Mod Manager Download → Slow Download).
    pub async fn download_links(
        &self,
        game_domain: &str,
        mod_id: u32,
        file_id: u32,
        free_ticket: Option<(&str, u64)>,
    ) -> AppResult<Vec<DownloadLink>> {
        let mut path = format!(
            "/games/{game_domain}/mods/{mod_id}/files/{file_id}/download_link.json"
        );
        if let Some((key, expires)) = free_ticket {
            let encoded_key = urlencoding_simple(key);
            path.push_str(&format!("?key={encoded_key}&expires={expires}"));
        }

        let v = match self.get_json(&path).await {
            Ok(v) => v,
            Err(e) => {
                let msg = e.to_string();
                if free_ticket.is_none()
                    && (msg.contains("403")
                        || msg.contains("permission")
                        || msg.contains("premium"))
                {
                    return Err(AppError::msg(
                        "Direct API downloads require Nexus Premium. Use “Download through Nexus” (Mod Manager Download → Slow Download) instead.",
                    ));
                }
                if free_ticket.is_some()
                    && (msg.contains("400") || msg.contains("key") || msg.contains("expire"))
                {
                    return Err(AppError::msg(
                        "The temporary Nexus download key is invalid or expired. Open the file on Nexus again and choose Mod Manager Download → Slow Download.",
                    ));
                }
                return Err(e);
            }
        };

        let arr = if let Some(a) = v.as_array() {
            a.clone()
        } else {
            return Err(AppError::msg(
                "Could not get a download link from Nexus. Free accounts need an NXM link from the website (Mod Manager Download).",
            ));
        };

        let mut links = Vec::new();
        for item in arr {
            if let Some(uri) = item
                .get("URI")
                .or_else(|| item.get("uri"))
                .and_then(|x| x.as_str())
            {
                links.push(DownloadLink {
                    name: item
                        .get("name")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    short_name: item
                        .get("short_name")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    uri: uri.to_string(),
                });
            }
        }

        if links.is_empty() {
            return Err(AppError::msg(
                "No download mirrors returned. Try again, or open the mod on Nexus and use Import ZIP.",
            ));
        }
        Ok(links)
    }

    pub async fn download_file_to(&self, url: &str, dest: &Path) -> AppResult<PathBuf> {
        crate::log::info(format!(
            "Downloading to {} (url host prefix: {})",
            dest.display(),
            url.chars().take(80).collect::<String>()
        ));

        // Free/slow CDN mirrors can take a long time; do not use the short API timeout.
        let res = self
            .http
            .get(url)
            .timeout(Duration::from_secs(60 * 60))
            .send()
            .await
            .map_err(|e| {
                crate::log::error(format!("HTTP download request failed: {e}"));
                AppError::from(e)
            })?;
        if !res.status().is_success() {
            let msg = format!("Download failed with status {}", res.status());
            crate::log::error(&msg);
            return Err(AppError::msg(msg));
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                crate::log::io_ctx(parent, "creating download directory", e)
            })?;
        }

        // Unique temp name avoids races when two installs target the same cache file.
        let temporary = dest.with_extension(format!(
            "part.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));

        let result = async {
            let mut file = tokio::fs::File::create(&temporary).await.map_err(|e| {
                crate::log::io_ctx(&temporary, "creating temporary download file", e)
            })?;
            let mut stream = res.bytes_stream();
            let mut written: u64 = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    crate::log::error(format!("Download stream error: {e}"));
                    AppError::from(e)
                })?;
                written = written.saturating_add(chunk.len() as u64);
                file.write_all(&chunk).await.map_err(|e| {
                    crate::log::io_ctx(&temporary, "writing download chunk", e)
                })?;
            }
            file.flush()
                .await
                .map_err(|e| crate::log::io_ctx(&temporary, "flushing download file", e))?;
            drop(file);

            crate::log::info(format!(
                "Download finished ({written} bytes) → promoting {}",
                temporary.display()
            ));

            // Replace destination safely. Prefer rename; on Windows fall back to
            // copy+remove if rename fails (AV locks, share violations, etc.).
            promote_download(&temporary, dest).await?;
            Ok(dest.to_path_buf())
        }
        .await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }
}

async fn promote_download(temporary: &Path, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        match tokio::fs::remove_file(dest).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(crate::log::io_ctx(dest, "removing previous cached download", e));
            }
        }
    }

    match tokio::fs::rename(temporary, dest).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            crate::log::warn(format!(
                "rename {} → {} failed ({e}); trying copy fallback",
                temporary.display(),
                dest.display()
            ));
        }
    }

    tokio::fs::copy(temporary, dest).await.map_err(|e| {
        crate::log::io_ctx(dest, "copying completed download into cache", e)
    })?;
    match tokio::fs::remove_file(temporary).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            crate::log::warn(format!(
                "Could not remove temp download {}: {e}",
                temporary.display()
            ));
        }
    }
    Ok(())
}

fn parse_mod_list(v: &Value) -> Vec<NexusModSummary> {
    let arr = if let Some(a) = v.as_array() {
        a.clone()
    } else if let Some(a) = v.get("results").and_then(|x| x.as_array()) {
        a.clone()
    } else if let Some(a) = v.get("mods").and_then(|x| x.as_array()) {
        a.clone()
    } else {
        return Vec::new();
    };

    arr.into_iter()
        .filter_map(|m| {
            let mod_id = m.get("mod_id").and_then(|x| x.as_u64())? as u32;
            Some(NexusModSummary {
                mod_id,
                name: m
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                summary: m
                    .get("summary")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                picture_url: m
                    .get("picture_url")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                author: m
                    .get("author")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                version: m
                    .get("version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                endorsements: m.get("endorsement_count").and_then(|x| x.as_u64()),
                downloads: m
                    .get("mod_downloads")
                    .and_then(|x| x.as_u64())
                    .or_else(|| m.get("downloads").and_then(|x| x.as_u64())),
                category_id: m
                    .get("category_id")
                    .and_then(|x| x.as_u64())
                    .map(|n| n as u32),
                created_time: m
                    .get("created_time")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                updated_time: m
                    .get("updated_time")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                available: m.get("available").and_then(|x| x.as_bool()).unwrap_or(true),
            })
        })
        .collect()
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn nexus_mod_page_url(mod_id: u32) -> String {
    format!("https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}")
}

/// File tab URL that nudges the browser toward Mod Manager Download (`nmm=1`).
pub fn nexus_file_page_url(mod_id: u32, file_id: u32) -> String {
    format!(
        "https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}?tab=files&file_id={file_id}&nmm=1"
    )
}
