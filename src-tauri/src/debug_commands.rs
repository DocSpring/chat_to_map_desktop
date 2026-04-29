//! Tauri commands for the hidden debug panel (5-click logo to reveal).
//!
//! These let testers swap the WEB host (chattomap.com results page) and the
//! API host (Convex HTTP actions) at runtime so a release-built binary can be
//! pointed at staging/local without rebuilding. Custom HTTP headers can also
//! be injected for things like Cloudflare Access tokens.
//!
//! All commands operate on `crate::AppState` which is held by Tauri's managed
//! state container.

use std::collections::HashMap;

use crate::AppState;

/// Set the WEB host URL override — affects the results page link only.
#[tauri::command]
pub fn set_server_host(state: tauri::State<AppState>, host: Option<String>) {
    let mut override_host = state.server_host_override.lock().unwrap();
    eprintln!("[set_server_host] Setting host override to: {:?}", host);
    *override_host = host;
}

/// Get the current WEB host URL (with override applied) — the host the
/// browser is opened to for the results page (chattomap.com).
#[tauri::command]
pub fn get_server_host(state: tauri::State<AppState>) -> String {
    let override_host = state.server_host_override.lock().unwrap();
    match override_host.as_ref() {
        Some(host) if !host.is_empty() => host.clone(),
        _ => chat_to_map_desktop::upload::WEB_BASE_URL.to_string(),
    }
}

/// Set the API host URL override (Convex HTTP actions; for debugging).
#[tauri::command]
pub fn set_api_host(state: tauri::State<AppState>, host: Option<String>) {
    let mut override_host = state.api_host_override.lock().unwrap();
    eprintln!("[set_api_host] Setting API host override to: {:?}", host);
    *override_host = host;
}

/// Get the current API host URL (with override applied) — Convex HTTP actions.
#[tauri::command]
pub fn get_api_host(state: tauri::State<AppState>) -> String {
    let override_host = state.api_host_override.lock().unwrap();
    match override_host.as_ref() {
        Some(host) if !host.is_empty() => host.clone(),
        _ => chat_to_map_desktop::upload::API_BASE_URL.to_string(),
    }
}

/// Set custom headers for API requests (for debugging).
#[tauri::command]
pub fn set_custom_headers(state: tauri::State<AppState>, headers: HashMap<String, String>) {
    let mut custom_headers = state.custom_headers.lock().unwrap();
    eprintln!("[set_custom_headers] Setting {} headers", headers.len());
    *custom_headers = headers;
}
