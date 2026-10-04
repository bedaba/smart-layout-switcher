use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub enabled: bool,
    pub switch_layout: bool,
    pub launch_at_login: bool,
    pub confidence: f32,
    pub excluded_apps: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            switch_layout: true,
            launch_at_login: false,
            confidence: 0.8,
            excluded_apps: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub platform: String,
    pub supported: bool,
    pub permission: String,
    pub message: String,
    pub layout: Option<String>,
}

#[derive(Clone, Default)]
struct SharedState(Arc<Mutex<AppSettings>>);

#[tauri::command]
fn get_runtime_status() -> RuntimeStatus {
    platform::status()
}

#[tauri::command]
fn update_settings(
    state: tauri::State<'_, SharedState>,
    mut settings: AppSettings,
) -> RuntimeStatus {
    settings.confidence = settings.confidence.clamp(0.6, 1.0);
    settings.excluded_apps = settings
        .excluded_apps
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .take(100)
        .collect();
    if let Ok(mut current) = state.0.lock() {
        *current = settings.clone();
    }
    platform::set_settings(settings);
    platform::status()
}

fn icon_pixels() -> tauri::image::Image<'static> {
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let dx = x - 16;
            let dy = y - 16;
            if dx * dx + dy * dy <= 225 {
                let i = ((y * 32 + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[119, 170, 255, 255]);
                if (9..23).contains(&x) && (10..13).contains(&y)
                    || (9..23).contains(&x) && (19..22).contains(&y)
                {
                    rgba[i..i + 4].copy_from_slice(&[17, 27, 42, 255]);
                }
                if (10..13).contains(&x) && (10..22).contains(&y)
                    || (19..22).contains(&x) && (10..22).contains(&y)
                {
                    rgba[i..i + 4].copy_from_slice(&[17, 27, 42, 255]);
                }
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 32, 32)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(SharedState::default())
        .invoke_handler(tauri::generate_handler![
            get_runtime_status,
            update_settings
        ])
        .setup(|app| {
            let shared = app.state::<SharedState>().0.clone();
            platform::start(shared, app.handle().clone());

            let show = MenuItem::with_id(app, "show", "فتح بدّلها", true, None::<&str>)?;
            let toggle = MenuItem::with_id(
                app,
                "toggle",
                "إيقاف أو استئناف التصحيح",
                true,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = PredefinedMenuItem::quit(app, Some("إنهاء بدّلها"))?;
            let menu = Menu::with_items(app, &[&show, &toggle, &separator, &quit])?;
            TrayIconBuilder::new()
                .icon(icon_pixels())
                .tooltip("بدّلها — تصحيح تخطيط الكتابة")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "toggle" => {
                        let state = app.state::<SharedState>();
                        if let Ok(mut cfg) = state.0.lock() {
                            cfg.enabled = !cfg.enabled;
                            let updated = cfg.clone();
                            platform::set_settings(updated);
                            let _ = app.emit("badelha-enabled", cfg.enabled);
                        };
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start Badelha");
}

mod platform;
