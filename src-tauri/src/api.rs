/*!
 * Hand-written API client for the ChatToMap SaaS upload endpoints.
 *
 * The desktop touches exactly two routes (`/api/upload/presign` and
 * `/api/upload/complete`) plus a direct PUT to a Convex storage URL. Replacing
 * the previous progenitor codegen with ~150 lines of `reqwest` removes the
 * dependency on a hand-maintained OpenAPI spec.
 *
 * Authentication: each request carries `X-Desktop-Signature` and
 * `X-Desktop-Timestamp` headers computed from
 * `HMAC-SHA256(DESKTOP_UPLOAD_SHARED_SECRET, "<timestamp>:<bound_value>")`.
 * For `presign`, the bound value is `content_length`. For `complete`, it is
 * the `storage_id` returned by the Convex storage upload. The server skips
 * Turnstile when the signature validates.
 */

use std::collections::HashMap;

use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const DESKTOP_SIGNATURE_HEADER: &str = "X-Desktop-Signature";
pub const DESKTOP_TIMESTAMP_HEADER: &str = "X-Desktop-Timestamp";

/// Secret used to sign upload requests. Baked into the binary at compile time.
/// In dev/test builds where the env var isn't set, we fall back to an empty
/// string and let the server reject the request with a clear error.
pub const DESKTOP_UPLOAD_SHARED_SECRET: &str = match option_env!("DESKTOP_UPLOAD_SHARED_SECRET") {
    Some(value) => value,
    None => "",
};

/// Per-request locale information forwarded to the SaaS for results presentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientLocale {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadCompleteRequest {
    pub storage_id: String,
    pub upload_platform: String, // "imessage"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_locale: Option<ClientLocale>,
    pub visitor_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresignData {
    pub upload_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadCompleteData {
    pub chat_upload_id: String,
    pub chat_analysis_id: String,
    pub status: String,
    #[serde(default)]
    pub job_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConvexStorageUploadResponse {
    #[serde(rename = "storageId")]
    pub storage_id: String,
}

/// API client for the ChatToMap SaaS upload endpoints.
pub struct ApiClient {
    base_url: String,
    http: reqwest::Client,
    secret: String,
    extra_headers: HeaderMap,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_secret(base_url, DESKTOP_UPLOAD_SHARED_SECRET.to_string())
    }

    pub fn with_secret(base_url: impl Into<String>, secret: String) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::new(),
            secret,
            extra_headers: HeaderMap::new(),
        }
    }

    /// Inject extra headers (used by the dev panel to spoof auth).
    pub fn with_extra_headers(mut self, headers: &HashMap<String, String>) -> Self {
        for (name, value) in headers {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                self.extra_headers.insert(header_name, header_value);
            }
        }
        self
    }

    pub async fn upload_presign(&self, content_length: u64) -> Result<PresignData, String> {
        let timestamp = current_unix_timestamp();
        let signature = sign_payload(&self.secret, &format!("{timestamp}:{content_length}"))
            .map_err(|e| format!("Failed to sign request: {e}"))?;
        let body = serde_json::json!({ "content_length": content_length });
        let url = format!("{}/api/upload/presign", self.base_url);

        let response = self
            .post(&url, &body, &timestamp, &signature)
            .await
            .map_err(|e| format!("presign request failed: {e}"))?;
        unwrap_api_response(response, "presign").await
    }

    pub async fn upload_complete(
        &self,
        body: UploadCompleteRequest,
    ) -> Result<UploadCompleteData, String> {
        let timestamp = current_unix_timestamp();
        let signature = sign_payload(&self.secret, &format!("{timestamp}:{}", body.storage_id))
            .map_err(|e| format!("Failed to sign request: {e}"))?;
        let url = format!("{}/api/upload/complete", self.base_url);
        let response = self
            .post(&url, &body, &timestamp, &signature)
            .await
            .map_err(|e| format!("complete request failed: {e}"))?;
        unwrap_api_response(response, "complete").await
    }

    async fn post<B: Serialize + ?Sized>(
        &self,
        url: &str,
        body: &B,
        timestamp: &str,
        signature: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let mut headers = self.extra_headers.clone();
        if let Ok(value) = HeaderValue::from_str(signature) {
            headers.insert(DESKTOP_SIGNATURE_HEADER, value);
        }
        if let Ok(value) = HeaderValue::from_str(timestamp) {
            headers.insert(DESKTOP_TIMESTAMP_HEADER, value);
        }
        self.http.post(url).headers(headers).json(body).send().await
    }
}

async fn unwrap_api_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    context: &str,
) -> Result<T, String> {
    let status = response.status();
    let body_text = response
        .text()
        .await
        .map_err(|e| format!("{context}: failed to read response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "{context} failed ({}): {}",
            status,
            truncate(&body_text, 200)
        ));
    }
    // Parse into a generic Value first so we don't impose Default on T.
    let raw: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("{context}: invalid JSON response: {e}"))?;
    let success = raw
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        let error = raw
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{context} returned success=false"));
        return Err(error);
    }
    let data = raw
        .get("data")
        .ok_or_else(|| format!("{context}: success response missing `data` field"))?;
    serde_json::from_value(data.clone())
        .map_err(|e| format!("{context}: failed to deserialize data: {e}"))
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

fn current_unix_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.to_string()
}

/// Compute the hex-encoded HMAC-SHA256 of `payload` keyed by `secret`.
pub fn sign_payload(secret: &str, payload: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("invalid HMAC key: {e}"))?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_payload_is_deterministic() {
        let a = sign_payload("secret", "1700000000:42").unwrap();
        let b = sign_payload("secret", "1700000000:42").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn sign_payload_changes_with_input() {
        let a = sign_payload("secret", "1700000000:42").unwrap();
        let b = sign_payload("secret", "1700000001:42").unwrap();
        let c = sign_payload("secret", "1700000000:43").unwrap();
        let d = sign_payload("other-secret", "1700000000:42").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn sign_payload_matches_node_crypto() {
        // Cross-checked against Node's crypto.createHmac to ensure the SaaS
        // (which signs in WebCrypto) and the desktop (which signs in Rust)
        // agree on bytes.
        let actual = sign_payload("test-secret-do-not-use", "1700000000:99").unwrap();
        assert_eq!(
            actual,
            "8eaf0f78db5e93514a1366f9e3b3d9c9e79b6ed1103fe4bc2f058e0ae5c2e4de"
        );
    }

    #[test]
    fn upload_complete_request_serializes_with_required_fields() {
        let req = UploadCompleteRequest {
            storage_id: "store-123".to_string(),
            upload_platform: "imessage".to_string(),
            original_filename: Some("export.zip".to_string()),
            client_locale: Some(ClientLocale {
                timezone: Some("Pacific/Auckland".to_string()),
                language: Some("en-NZ".to_string()),
            }),
            visitor_id: "visitor-abc".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["storage_id"], "store-123");
        assert_eq!(json["upload_platform"], "imessage");
        assert_eq!(json["original_filename"], "export.zip");
        assert_eq!(json["client_locale"]["timezone"], "Pacific/Auckland");
        assert_eq!(json["visitor_id"], "visitor-abc");
    }

    #[test]
    fn upload_complete_request_omits_optional_fields_when_none() {
        let req = UploadCompleteRequest {
            storage_id: "x".to_string(),
            upload_platform: "imessage".to_string(),
            original_filename: None,
            client_locale: None,
            visitor_id: "v".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("original_filename").is_none());
        assert!(json.get("client_locale").is_none());
    }
}
