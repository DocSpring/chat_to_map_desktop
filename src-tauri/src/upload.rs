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

/// Server base URL
#[cfg(debug_assertions)]
const SERVER_BASE_URL: &str = "http://localhost:5173";

#[cfg(not(debug_assertions))]
const SERVER_BASE_URL: &str = "https://chattomap.com";

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
        return Err(format!("Server error {}: {}", status, body));
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
        return Err(format!("Upload failed {}: {}", status, body));
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
        return Err(format!("Create job failed {}: {}", status, body));
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
}
