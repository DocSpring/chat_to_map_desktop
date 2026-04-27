/*!
 * Upload functionality for ChatToMap server.
 *
 * Flow (Convex backend):
 *   1. POST /api/upload/presign  -> { upload_url }
 *   2. PUT  upload_url           -> Convex storage returns { storageId }
 *   3. POST /api/upload/complete -> { chat_analysis_id, job_token, ... }
 *
 * Each presign / complete request carries an HMAC signature so the SaaS
 * backend can skip Turnstile (the desktop app cannot run a Turnstile widget).
 * See `src/api.rs` for the signing helper.
 */

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use uuid::Uuid;

use crate::api::{
    ApiClient, ConvexStorageUploadResponse, UploadCompleteData, UploadCompleteRequest,
};

// =============================================================================
// Public types
// =============================================================================

/// Result of the presign step (just the upload URL).
#[derive(Debug)]
pub struct PresignResponse {
    pub upload_url: String,
}

/// Result of completing the upload — the IDs we need to build the results URL.
#[derive(Debug, Clone)]
pub struct CreateJobResponse {
    pub chat_upload_id: String,
    pub chat_analysis_id: String,
    pub status: String,
    pub job_token: Option<String>,
}

impl From<UploadCompleteData> for CreateJobResponse {
    fn from(data: UploadCompleteData) -> Self {
        Self {
            chat_upload_id: data.chat_upload_id,
            chat_analysis_id: data.chat_analysis_id,
            status: data.status,
            job_token: data.job_token,
        }
    }
}

/// Progress callback for the PUT step.
pub type UploadProgressCallback = Box<dyn Fn(u8, String) + Send + Sync>;

// =============================================================================
// Configuration
// =============================================================================

#[cfg(feature = "dev-server")]
pub const SERVER_BASE_URL: &str = "http://localhost:5173";

#[cfg(not(feature = "dev-server"))]
pub const SERVER_BASE_URL: &str = "https://chattomap.com";

const VISITOR_ID_FILENAME: &str = "visitor_id.txt";

// =============================================================================
// Visitor ID — persisted UUID for this install
// =============================================================================

/// Read or create the per-install visitor ID. Stored as a plain UUID v4 string
/// in `<app_local_data_dir>/visitor_id.txt`. Best-effort — if persistence
/// fails (e.g. read-only mount), generates a fresh UUID for this run.
pub fn read_or_create_visitor_id(app_local_data_dir: &Path) -> String {
    let path = app_local_data_dir.join(VISITOR_ID_FILENAME);
    if let Ok(mut file) = File::open(&path) {
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_ok() {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(app_local_data_dir);
    if let Ok(mut file) = File::create(&path) {
        let _ = file.write_all(id.as_bytes());
    }
    id
}

// =============================================================================
// Client builder
// =============================================================================

fn build_client(
    host_override: Option<&str>,
    custom_headers: &HashMap<String, String>,
) -> ApiClient {
    let base_url = host_override.unwrap_or(SERVER_BASE_URL);
    ApiClient::new(base_url).with_extra_headers(custom_headers)
}

pub fn results_base_url(host_override: Option<&str>) -> String {
    host_override.unwrap_or(SERVER_BASE_URL).to_string()
}

// =============================================================================
// Presign + PUT + complete
// =============================================================================

pub async fn get_presigned_url(
    content_length: u64,
    host_override: Option<&str>,
    custom_headers: &HashMap<String, String>,
) -> Result<PresignResponse, String> {
    let client = build_client(host_override, custom_headers);
    let data = client.upload_presign(content_length).await?;
    Ok(PresignResponse {
        upload_url: data.upload_url,
    })
}

/// Upload the zip to the presigned Convex storage URL and return the
/// `storageId` that Convex assigned.
pub async fn upload_file(
    zip_path: &Path,
    upload_url: &str,
    progress_callback: Option<UploadProgressCallback>,
) -> Result<String, String> {
    let emit_progress = |percent: u8, message: String| {
        if let Some(ref cb) = progress_callback {
            cb(percent, message);
        }
    };

    emit_progress(0, "Reading export file...".to_string());

    let mut file = File::open(zip_path).map_err(|e| format!("Failed to open zip file: {e}"))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read zip file: {e}"))?;

    let file_size = buffer.len();
    emit_progress(10, format!("Uploading {}...", format_size(file_size)));

    let http_client = reqwest::Client::new();
    let response = http_client
        .post(upload_url)
        .header("Content-Type", "application/zip")
        .header("Content-Length", file_size)
        .body(buffer)
        .send()
        .await
        .map_err(|e| format!("Failed to upload file: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Upload failed {}: {}",
            status,
            sanitize_error_body(&body)
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read upload response: {e}"))?;
    let parsed: ConvexStorageUploadResponse = serde_json::from_str(&body).map_err(|e| {
        format!(
            "Invalid storage response: {e} (body: {})",
            truncate(&body, 100)
        )
    })?;

    emit_progress(100, "Upload complete".to_string());
    Ok(parsed.storage_id)
}

pub async fn complete_upload(
    storage_id: &str,
    visitor_id: &str,
    original_filename: Option<&str>,
    host_override: Option<&str>,
    custom_headers: &HashMap<String, String>,
) -> Result<CreateJobResponse, String> {
    let client = build_client(host_override, custom_headers);
    let req = UploadCompleteRequest {
        storage_id: storage_id.to_string(),
        upload_platform: "imessage".to_string(),
        original_filename: original_filename.map(|s| s.to_string()),
        client_locale: None,
        visitor_id: visitor_id.to_string(),
    };
    let data = client.upload_complete(req).await?;
    Ok(data.into())
}

/// Build the user-facing results URL for a completed upload.
pub fn get_results_url(
    chat_analysis_id: &str,
    job_token: Option<&str>,
    host_override: Option<&str>,
) -> String {
    let base_url = host_override.unwrap_or(SERVER_BASE_URL);
    match job_token {
        Some(token) if !token.is_empty() => format!(
            "{}/processing/{}?token={}",
            base_url,
            chat_analysis_id,
            urlencoding(token)
        ),
        _ => format!("{}/processing/{}", base_url, chat_analysis_id),
    }
}

/// Tiny URL-encoder for the token query param. Tokens are opaque base64-ish
/// strings; we just escape characters that aren't URL-safe.
fn urlencoding(input: &str) -> String {
    const SAFE: &[u8] = b"-._~ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        if SAFE.contains(byte) {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", byte));
        }
    }
    out
}

// =============================================================================
// Helpers
// =============================================================================

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

fn sanitize_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(empty response)".to_string();
    }
    if trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<HTML")
    {
        return "Server returned an HTML error page".to_string();
    }
    if trimmed.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
                return error.to_string();
            }
            if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
                return message.to_string();
            }
        }
    }
    truncate(trimmed, 200)
}

fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn format_size_picks_the_right_unit() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn results_url_is_built_with_token() {
        let url = get_results_url(
            "analysis-abc",
            Some("tok-xyz"),
            Some("http://localhost:5173"),
        );
        assert_eq!(
            url,
            "http://localhost:5173/processing/analysis-abc?token=tok-xyz"
        );
    }

    #[test]
    fn results_url_omits_token_when_missing() {
        let url = get_results_url("analysis-abc", None, Some("http://localhost:5173"));
        assert_eq!(url, "http://localhost:5173/processing/analysis-abc");
    }

    #[test]
    fn results_url_url_encodes_special_chars_in_token() {
        let url = get_results_url("a", Some("foo bar+baz"), Some("https://x.test"));
        assert_eq!(url, "https://x.test/processing/a?token=foo%20bar%2Bbaz");
    }

    #[test]
    fn visitor_id_is_persisted_and_reused() {
        let dir = TempDir::new().unwrap();
        let first = read_or_create_visitor_id(dir.path());
        let second = read_or_create_visitor_id(dir.path());
        assert_eq!(first, second);
        // UUID v4 strings are 36 chars (32 hex + 4 hyphens).
        assert_eq!(first.len(), 36);
        // Stored on disk for inspection.
        let stored = std::fs::read_to_string(dir.path().join(VISITOR_ID_FILENAME)).unwrap();
        assert_eq!(stored.trim(), first);
    }

    #[test]
    fn visitor_id_creates_directory_if_missing() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("not").join("yet").join("there");
        let id = read_or_create_visitor_id(&nested);
        assert_eq!(id.len(), 36);
        assert!(nested.join(VISITOR_ID_FILENAME).exists());
    }

    #[test]
    fn sanitize_error_body_extracts_json_error() {
        let body = r#"{"error":"Bad signature"}"#;
        assert_eq!(sanitize_error_body(body), "Bad signature");
    }

    #[test]
    fn sanitize_error_body_handles_html() {
        let body = "<!DOCTYPE html><html><head><title>boom</title></head></html>";
        assert_eq!(
            sanitize_error_body(body),
            "Server returned an HTML error page"
        );
    }

    #[test]
    fn sanitize_error_body_returns_empty_marker() {
        assert_eq!(sanitize_error_body(""), "(empty response)");
        assert_eq!(sanitize_error_body("   "), "(empty response)");
    }
}
