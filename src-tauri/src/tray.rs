//! The single tray icon shared by both accounts.

use crate::sync::{Engine, TickOpts};
use crate::window::{self, Rect, WinState};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Position, Size};

const TRAY_ID: &str = "main";

fn physical_rect(pos: Position, size: Size, scale: f64) -> Rect {
    let p = pos.to_physical::<i32>(scale);
    let s = size.to_physical::<u32>(scale);
    Rect::new(p.x, p.y, s.width as i32, s.height as i32)
}

pub fn build(app: &AppHandle, engine: Arc<Engine>) -> tauri::Result<()> {
    let refresh = MenuItem::with_id(app, "refresh", "Actualizar", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Configuración", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&refresh, &settings, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Claude Usage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "refresh" => {
                let (app, engine) = (app.clone(), engine.clone());
                tauri::async_runtime::spawn(async move {
                    engine.tick(&app, TickOpts { force_auth: true, probe: false }).await;
                });
            }
            "settings" => {
                window::show(app);
                let _ = app.emit("open-settings", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            let scale = app.get_webview_window(window::LABEL).and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
            match event {
                TrayIconEvent::Click { button, button_state, rect, .. } => {
                    app.state::<WinState>().remember_tray(physical_rect(rect.position, rect.size, scale));
                    if button == MouseButton::Left && button_state == MouseButtonState::Up {
                        window::toggle(app);
                    }
                }
                TrayIconEvent::Enter { rect, .. } | TrayIconEvent::Move { rect, .. } => {
                    app.state::<WinState>().remember_tray(physical_rect(rect.position, rect.size, scale));
                }
                _ => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
