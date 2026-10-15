//! Floating window lifecycle: placement above the tray icon (any taskbar edge,
//! any monitor), toggle, and hide-on-blur.

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position};

pub const LABEL: &str = "main";
const GAP: i32 = 8;
/// Clicking the tray icon blurs the window first, then delivers the click:
/// without this guard a click meant to *close* the window would reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(350);
/// Focus can flicker right after `show`; ignore blur for a moment.
const BLUR_GRACE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    fn right(&self) -> i32 {
        self.x + self.w
    }
    fn bottom(&self) -> i32 {
        self.y + self.h
    }
    fn cx(&self) -> i32 {
        self.x + self.w / 2
    }
    fn cy(&self) -> i32 {
        self.y + self.h / 2
    }
}

/// Top-left position (physical px) for a `win_w` x `win_h` window next to the
/// tray icon: placed on the side of the nearest monitor edge (= where the
/// taskbar is) and kept inside the monitor's work area.
pub fn place(tray: Rect, win_w: i32, win_h: i32, monitor: Rect, work: Rect) -> (i32, i32) {
    let edges = [
        (tray.cx() - monitor.x, Edge::Left),
        (monitor.right() - tray.cx(), Edge::Right),
        (tray.cy() - monitor.y, Edge::Top),
        (monitor.bottom() - tray.cy(), Edge::Bottom),
    ];
    let edge = edges.iter().min_by_key(|(d, _)| *d).map(|(_, e)| *e).unwrap_or(Edge::Bottom);
    let (x, y) = match edge {
        Edge::Bottom => (tray.cx() - win_w / 2, tray.y - win_h - GAP),
        Edge::Top => (tray.cx() - win_w / 2, tray.bottom() + GAP),
        Edge::Left => (tray.right() + GAP, tray.cy() - win_h / 2),
        Edge::Right => (tray.x - win_w - GAP, tray.cy() - win_h / 2),
    };
    (
        x.clamp(work.x + GAP, (work.right() - win_w - GAP).max(work.x + GAP)),
        y.clamp(work.y + GAP, (work.bottom() - win_h - GAP).max(work.y + GAP)),
    )
}

#[derive(Clone, Copy)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Default)]
pub struct WinState {
    last_tray: Mutex<Option<Rect>>,
    shown_at: Mutex<Option<Instant>>,
    blur_hidden_at: Mutex<Option<Instant>>,
}

impl WinState {
    pub fn remember_tray(&self, r: Rect) {
        if let Ok(mut g) = self.last_tray.lock() {
            *g = Some(r);
        }
    }
    fn tray(&self) -> Option<Rect> {
        self.last_tray.lock().ok().and_then(|g| *g)
    }
}

fn set<T>(slot: &Mutex<Option<T>>, v: Option<T>) {
    if let Ok(mut g) = slot.lock() {
        *g = v;
    }
}

fn recent(slot: &Mutex<Option<Instant>>, within: Duration) -> bool {
    slot.lock().ok().and_then(|g| *g).is_some_and(|t| t.elapsed() < within)
}

pub fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// Move the window next to the tray icon (or bottom-right of the primary
/// monitor when the icon position is unknown).
pub fn reposition(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else { return };
    let Ok(size) = win.outer_size() else { return };
    let (w, h) = (size.width as i32, size.height as i32);
    let state = app.state::<WinState>();

    let tray = state.tray();
    let monitor = tray
        .and_then(|t| app.monitor_from_point(f64::from(t.cx()), f64::from(t.cy())).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let m = Rect::new(monitor.position().x, monitor.position().y, monitor.size().width as i32, monitor.size().height as i32);
    let wa = monitor.work_area();
    let work = Rect::new(wa.position.x, wa.position.y, wa.size.width as i32, wa.size.height as i32);
    let tray = tray.unwrap_or_else(|| Rect::new(work.right() - 40, m.bottom(), 24, 24));

    let (x, y) = place(tray, w, h, m, work);
    let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

pub fn show(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else { return };
    let state = app.state::<WinState>();
    reposition(app);
    set(&state.shown_at, Some(Instant::now()));
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit("window-shown", ());
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
        let _ = app.emit("window-hidden", ());
    }
}

pub fn toggle(app: &AppHandle) {
    if is_visible(app) {
        hide(app);
    } else if !recent(&app.state::<WinState>().blur_hidden_at, REOPEN_GUARD) {
        show(app);
    }
}

pub fn on_blur(app: &AppHandle) {
    let state = app.state::<WinState>();
    if recent(&state.shown_at, BLUR_GRACE) || !is_visible(app) {
        return;
    }
    set(&state.blur_hidden_at, Some(Instant::now()));
    hide(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };
    const WORK_BOTTOM_TASKBAR: Rect = Rect { x: 0, y: 0, w: 1920, h: 1032 };

    #[test]
    fn bottom_taskbar_opens_above_the_icon() {
        let tray = Rect::new(1700, 1040, 24, 40);
        let (x, y) = place(tray, 380, 440, MON, WORK_BOTTOM_TASKBAR);
        assert_eq!(x, 1712 - 190);
        assert!(y + 440 <= tray.y);
    }

    #[test]
    fn window_is_clamped_to_the_work_area() {
        let tray = Rect::new(1900, 1040, 20, 40);
        let (x, _) = place(tray, 380, 440, MON, WORK_BOTTOM_TASKBAR);
        assert!(x + 380 <= 1920 - GAP);
    }

    #[test]
    fn top_taskbar_opens_below() {
        let tray = Rect::new(900, 0, 24, 40);
        let work = Rect::new(0, 48, 1920, 1032);
        let (_, y) = place(tray, 380, 440, MON, work);
        assert!(y >= tray.bottom());
    }

    #[test]
    fn left_and_right_taskbars() {
        let (x, _) = place(Rect::new(0, 500, 48, 24), 380, 440, MON, Rect::new(48, 0, 1872, 1080));
        assert!(x >= 48);
        let (x, _) = place(Rect::new(1872, 500, 48, 24), 380, 440, MON, Rect::new(0, 0, 1872, 1080));
        assert!(x + 380 <= 1872);
    }

    #[test]
    fn secondary_monitor_with_negative_origin() {
        let mon = Rect::new(-1920, 0, 1920, 1080);
        let work = Rect::new(-1920, 0, 1920, 1032);
        let tray = Rect::new(-300, 1040, 24, 40);
        let (x, y) = place(tray, 380, 440, mon, work);
        assert!(x >= mon.x && x + 380 <= mon.right());
        assert!(y + 440 <= tray.y);
    }

    #[test]
    fn click_that_blurred_the_window_does_not_reopen_it() {
        let slot = Mutex::new(Some(Instant::now()));
        assert!(recent(&slot, REOPEN_GUARD));
        let old = Mutex::new(Some(Instant::now() - Duration::from_secs(5)));
        assert!(!recent(&old, REOPEN_GUARD));
        assert!(!recent(&Mutex::new(None), REOPEN_GUARD));
    }
}
