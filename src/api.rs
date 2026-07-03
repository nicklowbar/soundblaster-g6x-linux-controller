use axum::{
    extract::{State, Json, Path},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::{BlasterXG6, DeviceRegistry, DeviceDescriptor};

fn run_sys_cmd(cmd: &str, args: &[&str]) -> Option<std::process::Output> {
    if std::path::Path::new("/.flatpak-info").exists() {
        let mut spawn_args = vec!["--host", cmd];
        spawn_args.extend_from_slice(args);
        std::process::Command::new("flatpak-spawn").args(&spawn_args).output().ok()
    } else {
        std::process::Command::new(cmd).args(args).output().ok()
    }
}


// Shared state — holds the registry of all connected devices
pub struct AppState {
    pub registry: Mutex<DeviceRegistry>,
}

#[derive(Serialize)]
pub struct MixerResponse {
    pub playback_vol: Option<f32>,
    pub playback_vol_l: Option<f32>,
    pub playback_vol_r: Option<f32>,
    pub playback_mute: Option<bool>,
    pub capture_vol: Option<f32>,
    pub capture_vol_l: Option<f32>,
    pub capture_vol_r: Option<f32>,
    pub capture_mute: Option<bool>,
}

fn get_pulse_device(prefix: &str, is_sink: bool) -> Option<String> {
    let mode = if is_sink { "sinks" } else { "sources" };
    let out = run_sys_cmd("pactl", &["list", "short", mode])?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if line.contains(prefix) && (is_sink || !line.contains(".monitor")) {
            if let Some(name) = line.split_whitespace().nth(1) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn get_pulse_vols(name: &str, is_source: bool) -> (Option<f32>, Option<f32>, Option<f32>) {
    let cmd = if is_source { "get-source-volume" } else { "get-sink-volume" };
    let out = run_sys_cmd("pactl", &[cmd, name]);
    if out.is_none() {
        return (None, None, None);
    }
    let s = String::from_utf8_lossy(&out.as_ref().unwrap().stdout);
    
    let mut left = None;
    let mut right = None;
    
    if let Some(front_left) = s.find("front-left:") {
        if let Some(slash1) = s[front_left..].find('/') {
            let rest1 = &s[front_left+slash1+1..];
            if let Some(pct1) = rest1.find('%') {
                left = rest1[..pct1].trim().parse::<f32>().ok().map(|v| v / 100.0);
            }
        }
    }
    if let Some(front_right) = s.find("front-right:") {
        if let Some(slash2) = s[front_right..].find('/') {
            let rest2 = &s[front_right+slash2+1..];
            if let Some(pct2) = rest2.find('%') {
                right = rest2[..pct2].trim().parse::<f32>().ok().map(|v| v / 100.0);
            }
        }
    }
    
    if left.is_none() && right.is_none() {
        if let Some(start) = s.find('/') {
            let rest = &s[start+1..];
            if let Some(end) = rest.find('%') {
                let v = rest[..end].trim().parse::<f32>().ok().map(|v| v / 100.0);
                return (v, v, v);
            }
        }
    }
    
    let avg = match (left, right) {
        (Some(l), Some(r)) => Some((l + r) / 2.0),
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        _ => None,
    };
    (avg, left, right)
}

fn get_pulse_mute(name: &str, is_source: bool) -> Option<bool> {
    let cmd = if is_source { "get-source-mute" } else { "get-sink-mute" };
    let out = run_sys_cmd("pactl", &[cmd, name])?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("yes") { Some(true) } else if s.contains("no") { Some(false) } else { None }
}

pub async fn get_mixer() -> impl IntoResponse {
    let mut map = std::collections::HashMap::new();
    let controls = ["Speaker", "Line In", "External Mic", "S/PDIF In", "What U Hear"];
    
    let g6_sink = get_pulse_device("Sound_BlasterX_G6", true);
    let g6_source = get_pulse_device("Sound_BlasterX_G6", false);

    for &ctrl in &controls {
        let output = run_sys_cmd("amixer", &["-c", "G6", "sget", ctrl]);
        if let Some(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut p_vol = None;
            let mut p_vol_l = None;
            let mut p_vol_r = None;
            let mut c_vol = None;
            let mut c_vol_l = None;
            let mut c_vol_r = None;
            let mut p_mute = None;
            let mut c_mute = None;

            let mut parsed_p_l = None;
            let mut parsed_p_r = None;
            let mut parsed_c_l = None;
            let mut parsed_c_r = None;

            for line in s.lines() {
                if line.contains("Front Left:") || (line.contains("Mono:") && !s.contains("Front Left:")) {
                    let parts: Vec<&str> = line.split("Capture").collect();
                    if let Some(p) = parts.get(0) {
                        if let Some(start) = p.find('[') {
                            if let Some(end) = p[start..].find('%') {
                                parsed_p_l = p[start+1..start+end].parse::<f32>().ok().map(|v| v / 100.0);
                            }
                        }
                        if p.contains("[off]") { p_mute = Some(true); } else if p.contains("[on]") { p_mute = Some(false); }
                    }
                    if let Some(p) = parts.get(1) {
                        if let Some(start) = p.find('[') {
                            if let Some(end) = p[start..].find('%') {
                                parsed_c_l = p[start+1..start+end].parse::<f32>().ok().map(|v| v / 100.0);
                            }
                        }
                        if p.contains("[off]") { c_mute = Some(true); } else if p.contains("[on]") { c_mute = Some(false); }
                    }
                }
                if line.contains("Front Right:") {
                    let parts: Vec<&str> = line.split("Capture").collect();
                    if let Some(p) = parts.get(0) {
                        if let Some(start) = p.find('[') {
                            if let Some(end) = p[start..].find('%') {
                                parsed_p_r = p[start+1..start+end].parse::<f32>().ok().map(|v| v / 100.0);
                            }
                        }
                    }
                    if let Some(p) = parts.get(1) {
                        if let Some(start) = p.find('[') {
                            if let Some(end) = p[start..].find('%') {
                                parsed_c_r = p[start+1..start+end].parse::<f32>().ok().map(|v| v / 100.0);
                            }
                        }
                    }
                }
            }
            
            p_vol_l = parsed_p_l;
            p_vol_r = parsed_p_r.or(parsed_p_l);
            p_vol = match (p_vol_l, p_vol_r) {
                (Some(l), Some(r)) => Some((l + r) / 2.0),
                (Some(l), None) => Some(l),
                _ => None,
            };
            
            c_vol_l = parsed_c_l;
            c_vol_r = parsed_c_r.or(parsed_c_l);
            c_vol = match (c_vol_l, c_vol_r) {
                (Some(l), Some(r)) => Some((l + r) / 2.0),
                (Some(l), None) => Some(l),
                _ => None,
            };

            // Sync with Pulse OS layer if available
            if ctrl == "Speaker" {
                if let Some(ref sink) = g6_sink {
                    let (avg, l, r) = get_pulse_vols(sink, false);
                    if let Some(v) = avg { p_vol = Some(v); }
                    if let Some(v) = l { p_vol_l = Some(v); }
                    if let Some(v) = r { p_vol_r = Some(v); }
                    if let Some(m) = get_pulse_mute(sink, false) { p_mute = Some(m); }
                }
            } else if ctrl == "External Mic" {
                if let Some(ref source) = g6_source {
                    let (avg, l, r) = get_pulse_vols(source, true);
                    if let Some(v) = avg { c_vol = Some(v); }
                    if let Some(v) = l { c_vol_l = Some(v); }
                    if let Some(v) = r { c_vol_r = Some(v); }
                    if let Some(m) = get_pulse_mute(source, true) { c_mute = Some(m); }
                }
            }

            map.insert(ctrl.to_string(), MixerResponse {
                playback_vol: p_vol,
                playback_vol_l: p_vol_l,
                playback_vol_r: p_vol_r,
                playback_mute: p_mute,
                capture_vol: c_vol,
                capture_vol_l: c_vol_l,
                capture_vol_r: c_vol_r,
                capture_mute: c_mute,
            });
        }
    }
    Json(map)
}

#[derive(Deserialize)]
pub struct MixerSetRequest {
    pub name: String,
    pub playback_vol: Option<f32>,
    pub playback_vol_l: Option<f32>,
    pub playback_vol_r: Option<f32>,
    pub playback_mute: Option<bool>,
    pub capture_vol: Option<f32>,
    pub capture_vol_l: Option<f32>,
    pub capture_vol_r: Option<f32>,
    pub capture_mute: Option<bool>,
}

pub async fn set_mixer(Json(payload): Json<MixerSetRequest>) -> impl IntoResponse {
    let g6_sink = get_pulse_device("Sound_BlasterX_G6", true);
    let g6_source = get_pulse_device("Sound_BlasterX_G6", false);

    // Playback volumes
    let has_p_l = payload.playback_vol_l.is_some();
    let has_p_r = payload.playback_vol_r.is_some();
    let has_p = payload.playback_vol.is_some();
    if has_p_l || has_p_r || has_p {
        // Find left and right values, default to existing payload.playback_vol or each other
        let p_base = payload.playback_vol.unwrap_or_else(|| payload.playback_vol_l.unwrap_or_else(|| payload.playback_vol_r.unwrap_or(0.0)));
        let p_l = payload.playback_vol_l.unwrap_or(p_base);
        let p_r = payload.playback_vol_r.unwrap_or(p_base);
        
        let pct_l = format!("{}%", (p_l * 100.0).round());
        let pct_r = format!("{}%", (p_r * 100.0).round());

        if payload.name == "Speaker" && g6_sink.is_some() {
            let _ = run_sys_cmd("pactl", &["set-sink-volume", g6_sink.as_ref().unwrap().as_str(), &pct_l, &pct_r]);
        } else {
            let pct = format!("{},{}", pct_l, pct_r); // amixer uses 40%,50% format
            let args = if payload.name == "Speaker" || payload.name == "What U Hear" {
                vec!["-c", "G6", "sset", &payload.name, "0", &pct]
            } else {
                vec!["-c", "G6", "sset", &payload.name, "0", &pct, "playback"]
            };
            let _ = run_sys_cmd("amixer", &args[..]);
        }
    }

    // Capture volumes
    let has_c_l = payload.capture_vol_l.is_some();
    let has_c_r = payload.capture_vol_r.is_some();
    let has_c = payload.capture_vol.is_some();
    if has_c_l || has_c_r || has_c {
        let c_base = payload.capture_vol.unwrap_or_else(|| payload.capture_vol_l.unwrap_or_else(|| payload.capture_vol_r.unwrap_or(0.0)));
        let c_l = payload.capture_vol_l.unwrap_or(c_base);
        let c_r = payload.capture_vol_r.unwrap_or(c_base);
        
        let pct_l = format!("{}%", (c_l * 100.0).round());
        let pct_r = format!("{}%", (c_r * 100.0).round());

        if payload.name == "External Mic" && g6_source.is_some() {
            let _ = run_sys_cmd("pactl", &["set-source-volume", g6_source.as_ref().unwrap().as_str(), &pct_l, &pct_r]);
        } else {
            let pct = format!("{},{}", pct_l, pct_r);
            let args = if payload.name == "Speaker" || payload.name == "What U Hear" {
                vec!["-c", "G6", "sset", &payload.name, "0", &pct]
            } else {
                vec!["-c", "G6", "sset", &payload.name, "0", &pct, "capture"]
            };
            let _ = run_sys_cmd("amixer", &args[..]);
        }
    }
    if let Some(m) = payload.playback_mute {
        let state = if m { "1" } else { "0" };
        if payload.name == "Speaker" && g6_sink.is_some() {
            let _ = run_sys_cmd("pactl", &["set-sink-mute", g6_sink.as_ref().unwrap().as_str(), state]);
        } else {
            let a_state = if m { "mute" } else { "unmute" };
            let args = if payload.name == "Speaker" || payload.name == "What U Hear" {
                vec!["-c", "G6", "sset", &payload.name, "0", a_state]
            } else {
                vec!["-c", "G6", "sset", &payload.name, "0", a_state, "playback"]
            };
            let _ = run_sys_cmd("amixer", &args[..]);
        }
    }
    if let Some(m) = payload.capture_mute {
        let state = if m { "1" } else { "0" };
        if payload.name == "External Mic" && g6_source.is_some() {
            let _ = run_sys_cmd("pactl", &["set-source-mute", g6_source.as_ref().unwrap().as_str(), state]);
        } else {
            let a_state = if m { "mute" } else { "unmute" };
            let args = if payload.name == "Speaker" || payload.name == "What U Hear" {
                vec!["-c", "G6", "sset", &payload.name, "0", a_state]
            } else {
                vec!["-c", "G6", "sset", &payload.name, "0", a_state, "capture"]
            };
            let _ = run_sys_cmd("amixer", &args[..]);
        }
    }
    StatusCode::OK.into_response()
}

// =============================================================================
// Device Registry API
// =============================================================================

/// GET /api/devices — list all connected devices
pub async fn get_devices(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let registry = state.registry.lock().await;
    let devices: Vec<DeviceDescriptor> = registry.descriptors();
    Json(devices)
}

/// GET /api/devices/:id — get device info and available operations
#[derive(Serialize)]
pub struct DeviceInfoResponse {
    pub device: DeviceDescriptor,
    pub operations: Vec<Operation>,
}

#[derive(Serialize)]
pub struct Operation {
    pub method: String,
    pub path: String,
    pub description: String,
}

pub async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> Response {
    let mut registry = state.registry.lock().await;

    let device = match registry.device_mut(id) {
        Some(d) => d,
        None => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Device not found", "id": id}))).into_response();
        }
    };

    let device_desc = DeviceDescriptor {
        id: device.id,
        name: device.device.product_string().unwrap_or("Unknown").to_string(),
        device_family: format!("{:?}", device.device_family),
        serial: device.device.serial_number().map(|s| s.to_string()),
        product_id: device.device.product_id(),
        interface: device.device.interface_number(),
    };

    let operations = vec![
        Operation {
            method: "GET".to_string(),
            path: format!("/api/devices/{}/status", id),
            description: "Get device status and features".to_string(),
        },
        Operation {
            method: "POST".to_string(),
            path: format!("/api/devices/{}/feature", id),
            description: "Set a device feature (toggle or slider)".to_string(),
        },
        Operation {
            method: "POST".to_string(),
            path: format!("/api/devices/{}/led", id),
            description: "Set LED on/off".to_string(),
        },
    ];

    Json(DeviceInfoResponse {
        device: device_desc,
        operations,
    }).into_response()
}

/// GET /api/devices/:id/status — get features and state for a specific device
#[derive(Serialize)]
pub struct DeviceStatusResponse {
    pub device: DeviceDescriptor,
    pub features: Vec<crate::Feature>,
    pub eq_bands: Option<[f32; 11]>,
}

pub async fn get_device_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> Response {
    let mut registry = state.registry.lock().await;

    let device = match registry.device_mut(id) {
        Some(d) => d,
        None => {
            return (StatusCode::NOT_FOUND, "Device not found").into_response();
        }
    };

    let device_desc = DeviceDescriptor {
        id: device.id,
        name: device.device.product_string().unwrap_or("Unknown").to_string(),
        device_family: format!("{:?}", device.device_family),
        serial: device.device.serial_number().map(|s| s.to_string()),
        product_id: device.device.product_id(),
        interface: device.device.interface_number(),
    };

    let features = device.features.clone();
    let eq_bands = device.get_ten_band_eq();

    Json(DeviceStatusResponse {
        device: device_desc,
        features,
        eq_bands,
    }).into_response()
}

#[derive(Deserialize)]
pub struct SetFeatureRequest {
    pub name: String,
    pub toggle: Option<bool>,
    pub slider: Option<f32>,
}

/// POST /api/devices/:id/feature — set a feature on a specific device
pub async fn set_device_feature(
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
    Json(payload): Json<SetFeatureRequest>,
) -> impl IntoResponse {
    let mut registry = state.registry.lock().await;

    let device = match registry.device_mut(id) {
        Some(d) => d,
        None => {
            return (StatusCode::NOT_FOUND, "Device not found").into_response();
        }
    };

    if let Some(toggle_val) = payload.toggle {
        if let Err(e) = device.set_feature(&payload.name, Some(toggle_val)) {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set feature: {}", e)).into_response();
        }
    }

    if let Some(slider_val) = payload.slider {
        if let Err(e) = device.set_slider(&payload.name, slider_val) {
             return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set slider: {}", e)).into_response();
        }
    }

    let default_profile = device.profile_path.join("default.json");
    if let Err(e) = device.save_profile(default_profile) {
        tracing::error!("Failed to save default profile: {}", e);
    }

    // Success
    StatusCode::OK.into_response()
}

// =============================================================================
// Legacy single-device API (backward compatible with existing frontend)
// Operates on first device in registry
// =============================================================================

#[derive(Serialize)]
pub struct StatusResponse {
    pub features: Vec<crate::Feature>,
    pub eq_bands: Option<[f32; 11]>,
    pub device_family: String,
    pub device_name: String,
}

/// GET /api/status — legacy endpoint for first device
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Response {
    let mut registry = state.registry.lock().await;

    // Operate on first device
    let device = match registry.first_device() {
        Some(d) => d,
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "No devices available").into_response();
        }
    };

    let features = device.features.clone();
    let eq_bands = device.get_ten_band_eq();
    let device_family = format!("{:?}", device.device_family);
    let device_name = device.device_family.name().to_string();

    Json(StatusResponse {
        features,
        eq_bands,
        device_family,
        device_name,
    }).into_response()
}

/// POST /api/feature — legacy endpoint for first device
pub async fn set_feature(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetFeatureRequest>,
) -> impl IntoResponse {
    let mut registry = state.registry.lock().await;

    // Operate on first device
    let device = match registry.device_mut(0) {
        Some(d) => d,
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "No devices available").into_response();
        }
    };

    if let Some(toggle_val) = payload.toggle {
        if let Err(e) = device.set_feature(&payload.name, Some(toggle_val)) {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set feature: {}", e)).into_response();
        }
    }

    if let Some(slider_val) = payload.slider {
        if let Err(e) = device.set_slider(&payload.name, slider_val) {
             return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set slider: {}", e)).into_response();
        }
    }

    let default_profile = device.profile_path.join("default.json");
    if let Err(e) = device.save_profile(default_profile) {
        tracing::error!("Failed to save default profile: {}", e);
    }

    // Success
    StatusCode::OK.into_response()
}

#[derive(Serialize)]
pub struct ApiInfo {
    pub endpoints: Vec<ApiEndpoint>,
}

#[derive(Serialize)]
pub struct ApiEndpoint {
    pub method: String,
    pub path: String,
    pub description: String,
}

/// GET /api — list available API endpoints
pub async fn get_api_info() -> impl IntoResponse {
    Json(ApiInfo {
        endpoints: vec![
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api".to_string(),
                description: "This API info".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api/devices".to_string(),
                description: "List all connected devices".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api/devices/:id".to_string(),
                description: "Get device info and available operations".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api/devices/:id/status".to_string(),
                description: "Get device status and features".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/devices/:id/feature".to_string(),
                description: "Set a device feature (toggle or slider)".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/devices/:id/led".to_string(),
                description: "Set LED on/off for a device".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api/status".to_string(),
                description: "Legacy: get first device status".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/feature".to_string(),
                description: "Legacy: set feature on first device".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/led".to_string(),
                description: "Legacy: set LED on first device".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/api/mixer/status".to_string(),
                description: "Get mixer status".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/mixer/feature".to_string(),
                description: "Set mixer feature".to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/api/show_window".to_string(),
                description: "Show the configuration window".to_string(),
            },
        ],
    }).into_response()
}

#[derive(Deserialize)]
pub struct LedRequest {
    pub on: bool,
}

/// POST /api/led — toggle LED on first device
pub async fn set_led(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LedRequest>,
) -> impl IntoResponse {
    let mut registry = state.registry.lock().await;

    // Operate on first device
    let device = match registry.device_mut(0) {
        Some(d) => d,
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "No devices available").into_response();
        }
    };

    if let Err(e) = device.set_feature("LED", Some(payload.on)) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set LED: {}", e)).into_response();
    }

    let default_profile = device.profile_path.join("default.json");
    if let Err(e) = device.save_profile(default_profile) {
        tracing::error!("Failed to save default profile: {}", e);
    }

    StatusCode::OK.into_response()
}

/// POST /api/devices/:id/led — toggle LED on specific device
pub async fn set_device_led(
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
    Json(payload): Json<LedRequest>,
) -> impl IntoResponse {
    let mut registry = state.registry.lock().await;

    let device = match registry.device_mut(id) {
        Some(d) => d,
        None => {
            return (StatusCode::NOT_FOUND, format!("Device {} not found", id)).into_response();
        }
    };

    if let Err(e) = device.set_feature("LED", Some(payload.on)) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set LED: {}", e)).into_response();
    }

    let default_profile = device.profile_path.join("default.json");
    if let Err(e) = device.save_profile(default_profile) {
        tracing::error!("Failed to save default profile: {}", e);
    }

    StatusCode::OK.into_response()
}

pub async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}

/// API fallback: return 404 JSON for unknown /api/* routes
pub async fn api_not_found(uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Not Found",
            "path": uri.path(),
        })),
    ).into_response()
}
