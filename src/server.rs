use axum::{
    body::Body,
    extract::{Path, Json, State, Request},
    http::{StatusCode, header, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::api::{self, AppState};
use crate::{BlasterXG6, DeviceFamily, DeviceRegistry};

pub static SHOW_WINDOW_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub async fn show_window() -> impl IntoResponse {
    SHOW_WINDOW_REQUEST.store(true, std::sync::atomic::Ordering::Relaxed);
    StatusCode::OK
}

#[derive(RustEmbed)]
#[folder = "frontend/build/"]
pub struct Assets;

pub async fn start_server(registry: DeviceRegistry) {
    let shared_state = Arc::new(AppState {
        registry: Mutex::new(registry),
    });

    // Spawn interrupt listener task for each G8 USB-1 device
    let shared_for_interrupt = Arc::clone(&shared_state);
    tokio::spawn(async move {
        loop {
            let mut all_interrupt_data: Vec<(usize, Vec<u8>)> = Vec::new();

            // Collect interrupt data from all G8 USB-1 devices
            {
                let mut registry = shared_for_interrupt.registry.lock().await;
                let device_ids: Vec<usize> = registry.devices()
                    .iter()
                    .filter(|d| d.supports_interrupts())
                    .map(|d| d.id)
                    .collect();

                for id in device_ids {
                    if let Some((_device, rx_opt)) = registry.device_interrupt_rx(id) {
                        if let Some(rx) = rx_opt {
                            // Try to receive without blocking
                            while let Ok(int_data) = rx.try_recv() {
                                tracing::trace!(target: "interrupt", "received interrupt data: {:02x?}", int_data);
                                all_interrupt_data.push((id, int_data));
                            }
                        }
                    }
                }
            }

            // Process interrupt data
            if !all_interrupt_data.is_empty() {
                let mut registry = shared_for_interrupt.registry.lock().await;
                for (id, int_data) in all_interrupt_data {
                    if let Some(device) = registry.device_mut(id) {
                        device.parse_interrupt(&int_data);
                    }
                }
            }

            // Small sleep to avoid busy looping
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Spawn background task to poll for device changes
    let registry_for_polling = Arc::clone(&shared_state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let mut registry = registry_for_polling.registry.lock().await;
            let old_descriptors = registry.descriptors();
            tracing::trace!(target: "registry", "Polling refresh: {} devices currently known", old_descriptors.len());
            registry.refresh();
            let new_descriptors = registry.descriptors();
            tracing::trace!(target: "registry", "Polling refresh complete: {} devices after refresh", new_descriptors.len());

            // Log device changes
            for desc in &new_descriptors {
                if !old_descriptors.iter().any(|d| d.id == desc.id) {
                    tracing::info!("Device connected: {} (ID: {})", desc.name, desc.id);
                }
            }
            for old_desc in &old_descriptors {
                if !new_descriptors.iter().any(|d| d.id == old_desc.id) {
                    tracing::info!("Device disconnected: {} (ID: {})", old_desc.name, old_desc.id);
                }
            }

            // Re-apply defaults to any new devices
            registry.apply_defaults();
        }
    });

    let app = Router::new()
        // Device management
        .route("/api/devices", get(api::get_devices))
        .route("/api/devices/:id/status", get(api::get_device_status))
        .route("/api/devices/:id/feature", post(api::set_device_feature))
        // Legacy single-device endpoints (operate on first device)
        .route("/api/status", get(api::get_status))
        .route("/api/feature", post(api::set_feature))
        // LED control
        .route("/api/devices/:id/led", post(api::set_device_led))
        .route("/api/led", post(api::set_led))
        // Mixer endpoints (device-agnostic)
        .route("/api/mixer/status", get(api::get_mixer))
        .route("/api/mixer/feature", post(api::set_mixer))
        // Window control
        .route("/api/show_window", post(show_window))
        // API root: list available endpoints
        .route("/api", get(api::get_api_info))
        // Static assets at /assets/*
        .route("/assets/*path", get(assets_handler))
        // SvelteKit app assets at /_app/*
        .route("/_app/*path", get(app_handler))
        // Web UI at root
        .route("/", get(index_handler))
        // Everything else → SPA fallback
        .fallback(spa_fallback)
        .with_state(shared_state)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3311));
    println!("Web server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Serve static assets from /assets/* → embedded files in assets/ folder
async fn assets_handler(Path(path): Path<String>) -> impl IntoResponse {
    let asset_path = format!("assets/{}", path);
    match Assets::get(&asset_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&asset_path).first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                content.data,
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// Serve SvelteKit app assets from /_app/* → embedded files at root
async fn app_handler(Path(path): Path<String>) -> impl IntoResponse {
    // Prepend _app/ since assets are embedded at root with _app prefix
    let full_path = format!("_app/{}", path);
    match Assets::get(&full_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                content.data,
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// Serve index.html for the web UI
async fn index_handler() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => {
            let mime = mime_guess::from_path("index.html").first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                content.data,
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// SPA fallback: /api/* → JSON 404, everything else → index.html for SPA
async fn spa_fallback(uri: Uri) -> impl IntoResponse {
    if uri.path().starts_with("/api") {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Not Found",
                "path": uri.path(),
            })),
        ).into_response();
    }

    // Serve index.html for SPA routing
    match Assets::get("index.html") {
        Some(content) => {
            let mime = mime_guess::from_path("index.html").first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                content.data,
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}
