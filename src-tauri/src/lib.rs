pub mod alerts;
pub mod cache;
pub mod claude_cli;
pub mod model;
pub mod paths;
pub mod provider;
pub mod settings;
pub mod snapshot;
pub mod sync;
pub mod tray;
pub mod window;

use model::{AppView, Slot};
use settings::Settings;
use std::sync::Arc;
use sync::{Engine, TickOpts};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_store::StoreExt;

const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings_v2";

fn slot_of(n: u8) -> Result<Slot, String> {
    Slot::new(n).ok_or_else(|| format!("cuenta {n} no existe"))
}

#[tauri::command]
fn get_state(engine: State<'_, Arc<Engine>>) -> AppView {
    engine.view()
}

#[tauri::command]
async fn refresh(app: AppHandle, engine: State<'_, Arc<Engine>>, probe: bool) -> Result<(), String> {
    engine.inner().clone().tick(&app, TickOpts { force_auth: true, probe }).await;
    Ok(())
}

#[tauri::command]
fn connect_account(app: AppHandle, engine: State<'_, Arc<Engine>>, slot: u8) -> Result<(), String> {
    engine.inner().connect(&app, slot_of(slot)?)
}

#[tauri::command]
async fn disconnect_account(app: AppHandle, engine: State<'_, Arc<Engine>>, slot: u8) -> Result<(), String> {
    engine.inner().clone().disconnect(&app, slot_of(slot)?).await
}

#[tauri::command]
fn save_settings(app: AppHandle, engine: State<'_, Arc<Engine>>, settings: Settings) -> Result<Settings, String> {
    let saved = engine.set_settings(settings);
    persist_settings(&app, &saved)?;
    apply_autostart(&app, saved.autostart);
    Ok(saved)
}

#[tauri::command]
fn fit_window(app: AppHandle, height: f64) {
    if let Some(win) = app.get_webview_window(window::LABEL) {
        let _ = win.set_size(tauri::LogicalSize::new(380.0, height.clamp(220.0, 680.0)));
        if window::is_visible(&app) {
            window::reposition(&app);
        }
    }
}

#[tauri::command]
fn fit_pill(app: AppHandle, width: f64, height: f64) {
    window::fit_pill(&app, width, height);
}

#[tauri::command]
fn toggle_window(app: AppHandle) {
    window::toggle(&app);
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    window::hide(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn persist_settings(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let store = app.store(SETTINGS_FILE).map_err(|e| e.to_string())?;
    store.set(SETTINGS_KEY, serde_json::to_value(s).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())
}

fn load_settings(app: &AppHandle) -> Settings {
    app.store(SETTINGS_FILE)
        .ok()
        .and_then(|store| store.get(SETTINGS_KEY))
        .and_then(|v| serde_json::from_value::<Settings>(v).ok())
        .unwrap_or_default()
        .sanitized()
}

/// Dev builds must not register the debug binary in the Windows Run key.
fn apply_autostart(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let manager = app.autolaunch();
    if enabled {
        // Always (re)write: keeps the Run entry pointing at the current exe
        // after an upgrade or move.
        let _ = manager.enable();
    } else if manager.is_enabled().unwrap_or(false) {
        let _ = manager.disable();
    }
}

/// `claude-usage-tray.exe --statusline <slot>`: helper invoked by Claude Code.
pub fn statusline(slot_arg: &str) -> i32 {
    snapshot::run_statusline(slot_arg)
}

pub fn run() {
    // Release builds have no console: keep every panic in crash.log.
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        let path = paths::base_dir().join("crash.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let bt = std::backtrace::Backtrace::force_capture();
            let _ = writeln!(f, "[{:?}] {info}\n{bt}\n", std::time::SystemTime::now());
        }
    }));
    tauri::Builder::default()
        // Must be first: a second launch just focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::show(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(window::WinState::default())
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh,
            connect_account,
            disconnect_account,
            save_settings,
            fit_window,
            fit_pill,
            toggle_window,
            hide_window,
            quit_app
        ])
        .on_window_event(|win, event| {
            if win.label() != window::LABEL {
                return;
            }
            match event {
                WindowEvent::Focused(false) => window::on_blur(win.app_handle()),
                // Closing only hides: the process and the tray icon live on.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    window::hide(win.app_handle());
                }
                _ => {}
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = load_settings(&handle);
            apply_autostart(&handle, settings.autostart);
            if app.store(SETTINGS_FILE).map(|s| s.get(SETTINGS_KEY).is_none()).unwrap_or(false) {
                let _ = persist_settings(&handle, &settings);
            }

            let exe = std::env::current_exe()?;
            let engine = Engine::new(paths::base_dir(), exe, settings);
            app.manage(engine.clone());
            tray::build(&handle, engine.clone())?;
            // Always start in the tray: WebView2 creation can surface the
            // window even with `visible: false` in the config.
            window::hide(&handle);
            window::build_pill(&handle)?;
            tauri::async_runtime::spawn(engine.run(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Claude Usage");
}
