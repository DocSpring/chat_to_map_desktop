/*!
 * Upload functionality for ChatToMap server
 *
 * Handles pre-signed URL fetching and file upload to R2.
 */

use std::{fs::File, io::Read, path::Path};

use reqwest::Client;
use serde::{Deserialize, Serialize};

// =============================================================================
// Types
// =============================================================================

/// Response from the pre-sign endpoint
#[derive(Debug, Deserialize)]
pub struct PresignResponse {
    /// Pre-signed upload URL
    pub upload_url: String,
    /// Job ID for tracking
    pub job_id: String,
    /// File key in storage
    pub file_key: String,
}

/// Request to create a job
#[derive(Debug, Serialize)]
struct CreateJobRequest {
    file_key: String,
    source: String,
    chat_count: usize,
    message_count: usize,
}

/// Response from the create job endpoint
#[derive(Debug, Deserialize)]
pub struct CreateJobResponse {
    pub job_id: String,
    pub status: String,
}

/// Progress callback for upload
pub type UploadProgressCallback = Box<dyn Fn(u8, String) + Send + Sync>;

// =============================================================================
// Configuration
// =============================================================================

/// Server base URL - use `--features dev-server` to point to localhost
#[cfg(feature = "dev-server")]
pub const SERVER_BASE_URL: &str = "http://localhost:5173";

#[cfg(not(feature = "dev-server"))]
pub const SERVER_BASE_URL: &str = "https://chattomap.com";

// =============================================================================
// Upload Implementation
// =============================================================================

/// Request a pre-signed upload URL from the server
pub async fn get_presigned_url() -> Result<PresignResponse, String> {
    let client = Client::new();
    let url = format!("{}/api/desktop/presign", SERVER_BASE_URL);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to request presigned URL: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Server error {}: {}",
            status,
            sanitize_error_body(&body)
        ));
    }

    response
        .json::<PresignResponse>()
        .await
        .map_err(|e| format!("Failed to parse presign response: {e}"))
}

/// Upload a file to the pre-signed URL
pub async fn upload_file(
    zip_path: &Path,
    upload_url: &str,
    progress_callback: Option<UploadProgressCallback>,
) -> Result<(), String> {
    let emit_progress = |percent: u8, message: String| {
        if let Some(ref cb) = progress_callback {
            cb(percent, message);
        }
    };

    emit_progress(0, "Reading export file...".to_string());

    // Read file into memory
    let mut file = File::open(zip_path).map_err(|e| format!("Failed to open zip file: {e}"))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read zip file: {e}"))?;

    let file_size = buffer.len();
    emit_progress(10, format!("Uploading {} bytes...", format_size(file_size)));

    // Upload to pre-signed URL
    let client = Client::new();
    let response = client
        .put(upload_url)
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

    emit_progress(100, "Upload complete".to_string());

    Ok(())
}

/// Notify server that upload is complete and create processing job
pub async fn create_job(
    file_key: &str,
    chat_count: usize,
    message_count: usize,
) -> Result<CreateJobResponse, String> {
    let client = Client::new();
    let url = format!("{}/api/desktop/job", SERVER_BASE_URL);

    let request = CreateJobRequest {
        file_key: file_key.to_string(),
        source: "imessage".to_string(),
        chat_count,
        message_count,
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to create job: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Create job failed {}: {}",
            status,
            sanitize_error_body(&body)
        ));
    }

    response
        .json::<CreateJobResponse>()
        .await
        .map_err(|e| format!("Failed to parse job response: {e}"))
}

/// Get the results page URL for a job
pub fn get_results_url(job_id: &str) -> String {
    format!("{}/processing/{}", SERVER_BASE_URL, job_id)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Sanitize an error response body for display
///
/// If the body looks like HTML, extract a meaningful message or return a generic error.
/// Otherwise, truncate and return the raw body.
fn sanitize_error_body(body: &str) -> String {
    let trimmed = body.trim();

    // Empty body
    if trimmed.is_empty() {
        return "(empty response)".to_string();
    }

    // Detect HTML content
    if trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<HTML")
    {
        // Try to extract title or meaningful content
        if let Some(title) = extract_html_title(trimmed) {
            return title;
        }
        return "Server returned an HTML error page".to_string();
    }

    // Try to parse as JSON error
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

    // Plain text - truncate if too long
    if trimmed.len() > 200 {
        format!("{}...", &trimmed[..200])
    } else {
        trimmed.to_string()
    }
}

/// Extract title from HTML content
fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower[start..].find("</title>") {
            let title_start = start + 7;
            let title = html[title_start..start + end].trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// Format file size for display
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

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 2 + 512 * 1024), "2.5 MB");
    }

    #[test]
    fn test_get_results_url() {
        let url = get_results_url("abc123");
        assert!(url.contains("abc123"));
        assert!(url.contains("/processing/"));
    }

    #[test]
    fn test_sanitize_error_body_empty() {
        assert_eq!(sanitize_error_body(""), "(empty response)");
        assert_eq!(sanitize_error_body("   "), "(empty response)");
    }

    #[test]
    fn test_sanitize_error_body_html() {
        let html =
            r#"<!DOCTYPE html><html><head><title>Not Found</title></head><body>...</body></html>"#;
        assert_eq!(sanitize_error_body(html), "Not Found");
    }

    #[test]
    fn test_sanitize_error_body_html_no_title() {
        let html = r#"<!DOCTYPE html><html><body>Error page</body></html>"#;
        assert_eq!(
            sanitize_error_body(html),
            "Server returned an HTML error page"
        );
    }

    #[test]
    fn test_sanitize_error_body_json_error() {
        let json = r#"{"error": "Invalid request"}"#;
        assert_eq!(sanitize_error_body(json), "Invalid request");
    }

    #[test]
    fn test_sanitize_error_body_json_message() {
        let json = r#"{"message": "Something went wrong"}"#;
        assert_eq!(sanitize_error_body(json), "Something went wrong");
    }

    #[test]
    fn test_sanitize_error_body_plain_text() {
        let text = "Connection refused";
        assert_eq!(sanitize_error_body(text), "Connection refused");
    }

    #[test]
    fn test_sanitize_error_body_truncates_long_text() {
        let long_text = "x".repeat(300);
        let result = sanitize_error_body(&long_text);
        assert!(result.ends_with("..."));
        assert!(result.len() < 210);
    }
}
