//! Floating window lifecycle: placement above the tray icon (any taskbar edge,
//! any monitor), toggle, and hide-on-blur.

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position};

pub const LABEL: &str = "main";
/// Borderless window docked in the tray corner that shows the usage as text.
pub const PILL_LABEL: &str = "pill";
const GAP: i32 = 8;
const PILL_MARGIN: i32 = 8;
pub const PILL_W: f64 = 164.0;
pub const PILL_H: f64 = 22.0;
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

// ---- usage pill ---------------------------------------------------------
//
// One pill per taskbar (primary + every secondary monitor), docked right
// against the tray buttons (left of the keyboard language / clock). A background thread re-reads the taskbars every
// second and only moves a pill when its slot actually changed.

/// Bottom-right corner of the work area (fallback when no taskbar can be
/// docked to). Windows tray icons are 16 px wide, so real text needs its own
/// window.
fn corner(work: Rect, w: i32, h: i32) -> (i32, i32) {
    (
        (work.right() - w - PILL_MARGIN).max(work.x + PILL_MARGIN),
        (work.bottom() - h - PILL_MARGIN).max(work.y + PILL_MARGIN),
    )
}

/// Slot inside the taskbar, immediately left of `limit` (the first tray
/// button: chevron / keyboard language, or the clock on secondary bars).
/// `None` when the bar is vertical, too small, or there is no room.
fn taskbar_slot(bar: Rect, limit: i32, w: i32, h: i32) -> Option<(i32, i32)> {
    if bar.w <= bar.h || bar.h < 24 || h > bar.h - 4 {
        return None;
    }
    let limit = limit.min(bar.right());
    let x = limit - PILL_MARGIN / 2 - w;
    if x < bar.x + PILL_MARGIN {
        return None;
    }
    Some((x, bar.y + (bar.h - h) / 2))
}

/// One taskbar, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub bar: Rect,
    /// Left edge of the first tray button, or the bar's right edge.
    pub limit: i32,
    pub hwnd: isize,
}

#[cfg(windows)]
mod taskbar {
    use super::{Bar, Rect};

    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    /// `APPBARDATA` from shellapi.h.
    #[repr(C)]
    struct AppBarData {
        cb_size: u32,
        hwnd: Hwnd,
        callback_message: u32,
        edge: u32,
        rc: WinRect,
        l_param: isize,
    }
    type Hwnd = *mut core::ffi::c_void;

    #[link(name = "shell32")]
    extern "system" {
        fn SHAppBarMessage(msg: u32, data: *mut AppBarData) -> usize;
    }

    extern "system" {
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, name: *const u16) -> Hwnd;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn GetClassNameW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> Hwnd;
        fn GetMonitorInfoW(monitor: Hwnd, info: *mut MonitorInfo) -> i32;
    }

    const ABM_GETSTATE: u32 = 4;
    const ABS_AUTOHIDE: usize = 0x1;
    const GW_HWNDPREV: u32 = 3;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const TRAY_CLASS: &str = "SystemTray.AccentButton";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Auto-hide taskbars are parked off-screen, so their rect cannot be used
    /// to dock anything: the pill falls back to the work-area corner instead.
    pub fn auto_hide() -> bool {
        let mut data = AppBarData {
            cb_size: std::mem::size_of::<AppBarData>() as u32,
            hwnd: std::ptr::null_mut(),
            callback_message: 0,
            edge: 0,
            rc: WinRect { left: 0, top: 0, right: 0, bottom: 0 },
            l_param: 0,
        };
        unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) as usize & ABS_AUTOHIDE != 0 }
    }

    fn rect(hwnd: Hwnd) -> Option<Rect> {
        let mut r = WinRect { left: 0, top: 0, right: 0, bottom: 0 };
        if hwnd.is_null() || unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
            return None;
        }
        Some(Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top))
    }

    fn child(parent: Hwnd, class: &str) -> Hwnd {
        let c = wide(class);
        unsafe { FindWindowExW(parent, std::ptr::null_mut(), c.as_ptr(), std::ptr::null()) }
    }

    /// Left edge of the first tray button (chevron, keyboard language,
    /// clock...). The Win11 taskbar is XAML, so the buttons are only visible
    /// to UI Automation; `TrayNotifyWnd` is the fallback.
    fn tray_left(uia: Option<&uiautomation::UIAutomation>, bar: Hwnd) -> Option<i32> {
        let from_uia = uia.and_then(|uia| {
            let root = uia.element_from_handle((bar as isize).into()).ok()?;
            let cond = uia
                .create_property_condition(uiautomation::types::UIProperty::ClassName, TRAY_CLASS.into(), None)
                .ok()?;
            let buttons = root.find_all(uiautomation::types::TreeScope::Descendants, &cond).ok()?;
            buttons
                .iter()
                .filter_map(|b| b.get_bounding_rectangle().ok())
                .filter(|r| r.get_right() > r.get_left())
                .map(|r| r.get_left())
                .min()
        });
        from_uia.or_else(|| rect(child(bar, "TrayNotifyWnd")).filter(|n| n.w > 0).map(|n| n.x))
    }

    /// Every visible taskbar, primary first. Empty when auto-hidden.
    pub fn bars(uia: Option<&uiautomation::UIAutomation>) -> Vec<Bar> {
        if auto_hide() {
            return Vec::new();
        }
        let mut hwnds = Vec::new();
        let primary = child(std::ptr::null_mut(), "Shell_TrayWnd");
        if !primary.is_null() {
            hwnds.push(primary);
        }
        let cls = wide("Shell_SecondaryTrayWnd");
        let mut after: Hwnd = std::ptr::null_mut();
        loop {
            after = unsafe { FindWindowExW(std::ptr::null_mut(), after, cls.as_ptr(), std::ptr::null()) };
            if after.is_null() {
                break;
            }
            hwnds.push(after);
        }
        hwnds
            .into_iter()
            .filter(|&h| unsafe { IsWindowVisible(h) } != 0)
            .filter_map(|h| {
                let bar = rect(h)?;
                let limit = tray_left(uia, h).filter(|&x| x > bar.x).unwrap_or(bar.right());
                Some(Bar { bar, limit, hwnd: h as isize })
            })
            .collect()
    }

    /// The taskbar joins the topmost band and gets painted over the pill when
    /// the user clicks it. Only in that case the pill is raised again, so it
    /// never jumps above other topmost surfaces by itself.
    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        monitor: WinRect,
        work: WinRect,
        flags: u32,
    }

    /// Monitor rect covered by the foreground window when it is fullscreen
    /// (games, video, F11). The desktop itself is ignored.
    pub fn fullscreen() -> Option<Rect> {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.is_null() {
                return None;
            }
            let mut buf = [0u16; 64];
            let n = GetClassNameW(fg, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
            let class = String::from_utf16_lossy(&buf[..n]);
            if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
                return None;
            }
            let win = rect(fg)?;
            let mut info = MonitorInfo {
                cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                monitor: WinRect { left: 0, top: 0, right: 0, bottom: 0 },
                work: WinRect { left: 0, top: 0, right: 0, bottom: 0 },
                flags: 0,
            };
            if GetMonitorInfoW(MonitorFromWindow(fg, 2), &mut info) == 0 {
                return None;
            }
            let m = info.monitor;
            let covers = win.x <= m.left && win.y <= m.top && win.right() >= m.right && win.bottom() >= m.bottom;
            covers.then(|| Rect::new(m.left, m.top, m.right - m.left, m.bottom - m.top))
        }
    }

    pub fn raise_if_covered(pill: isize, bars: &[Bar]) {
        let mut cur = unsafe { GetWindow(pill as Hwnd, GW_HWNDPREV) };
        for _ in 0..512 {
            if cur.is_null() {
                return;
            }
            if bars.iter().any(|b| b.hwnd == cur as isize) {
                unsafe {
                    SetWindowPos(
                        pill as Hwnd,
                        HWND_TOPMOST as Hwnd,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                return;
            }
            cur = unsafe { GetWindow(cur, GW_HWNDPREV) };
        }
    }
}

#[cfg(not(windows))]
mod taskbar {
    use super::Bar;
    pub fn bars(_uia: Option<&()>) -> Vec<Bar> {
        Vec::new()
    }
    pub fn fullscreen() -> Option<super::Rect> {
        None
    }
    pub fn auto_hide() -> bool {
        false
    }
}

/// Pill state shared by the dock thread and `fit_pill`.
struct Dock {
    /// Logical size requested by the webview.
    size: (f64, f64),
    bars: Vec<Bar>,
    /// Last physical rect applied per label: a pill only moves on change.
    applied: std::collections::HashMap<String, Rect>,
}

static DOCK: Mutex<Option<Dock>> = Mutex::new(None);

fn pill_label(i: usize) -> String {
    if i == 0 {
        PILL_LABEL.to_string()
    } else {
        format!("{PILL_LABEL}-{i}")
    }
}

fn pill_index(label: &str) -> Option<usize> {
    match label.strip_prefix(PILL_LABEL)? {
        "" => Some(0),
        rest => rest.strip_prefix('-')?.parse().ok(),
    }
}

fn pills(app: &AppHandle) -> Vec<tauri::WebviewWindow> {
    app.webview_windows().into_iter().filter(|(l, _)| pill_index(l).is_some()).map(|(_, w)| w).collect()
}

fn create_pill(app: &AppHandle, label: &str) -> Option<tauri::WebviewWindow> {
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("pill.html".into()))
        .title("Claude Usage")
        .inner_size(PILL_W, PILL_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .ok()
}

fn scale_at(app: &AppHandle, x: i32, y: i32) -> f64 {
    app.monitor_from_point(f64::from(x), f64::from(y))
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

/// Target rects (physical) for every pill: one per taskbar, or one in the
/// primary work-area corner when no taskbar can be docked to.
fn targets(app: &AppHandle, dock: &Dock) -> Vec<Rect> {
    let (lw, lh) = dock.size;
    let px = |s: f64| ((lw * s).round() as i32, (lh * s).round() as i32);
    let mut out: Vec<Rect> = dock
        .bars
        .iter()
        .filter_map(|b| {
            let (w, h) = px(scale_at(app, b.bar.cx(), b.bar.cy()));
            taskbar_slot(b.bar, b.limit, w, h).map(|(x, y)| Rect::new(x, y, w, h))
        })
        .collect();
    if out.is_empty() {
        if let Some(m) = app.primary_monitor().ok().flatten() {
            let (w, h) = px(m.scale_factor());
            let wa = m.work_area();
            let work = Rect::new(wa.position.x, wa.position.y, wa.size.width as i32, wa.size.height as i32);
            let (x, y) = corner(work, w, h);
            out.push(Rect::new(x, y, w, h));
        }
    }
    out
}

/// Create, move, show or hide the pills so they match the current taskbars.
/// Runs off the main thread: window creation waits on it.
fn apply(app: &AppHandle) {
    // Create missing pills before taking the lock: window creation waits on
    // the main thread, which may itself be waiting on the lock in `fit_pill`.
    let count = match DOCK.lock() {
        Ok(g) => match g.as_ref() {
            Some(d) => targets(app, d).len(),
            None => return,
        },
        Err(_) => return,
    };
    for i in 0..count {
        let label = pill_label(i);
        if app.get_webview_window(&label).is_none() {
            // `skip_taskbar` on the builder is lost when built off the main thread.
            if let Some(w) = create_pill(app, &label) {
                let _ = w.set_skip_taskbar(true);
            }
        }
    }

    let Ok(mut guard) = DOCK.lock() else { return };
    let Some(dock) = guard.as_mut() else { return };
    let targets = targets(app, dock);
    let panel_open = is_visible(app);
    let fullscreen = taskbar::fullscreen();
    for (i, r) in targets.iter().enumerate() {
        let label = pill_label(i);
        let Some(win) = app.get_webview_window(&label) else { continue };
        if dock.applied.get(&label) != Some(r) {
            // Size first: resizing a borderless window keeps its right edge
            // pinned on Windows, which shifted the pill after positioning.
            let _ = win.set_size(tauri::PhysicalSize::new(r.w as u32, r.h as u32));
            let _ = win.set_position(Position::Physical(PhysicalPosition::new(r.x, r.y)));
            dock.applied.insert(label, *r);
        }
        // A fullscreen app (game, video) owns its monitor: no pill on top.
        if fullscreen.is_some_and(|f| f.x <= r.x && r.x < f.right() && f.y <= r.y && r.y < f.bottom()) {
            if win.is_visible().unwrap_or(false) {
                let _ = win.hide();
            }
            continue;
        }
        if panel_open {
            continue;
        }
        if !win.is_visible().unwrap_or(true) {
            let _ = win.show();
        }
        #[cfg(windows)]
        if let Ok(hwnd) = win.hwnd() {
            taskbar::raise_if_covered(hwnd.0 as isize, &dock.bars);
        }
    }
    // Monitor unplugged: hide the pills that lost their taskbar.
    for win in pills(app) {
        if pill_index(win.label()).is_some_and(|i| i >= targets.len()) && win.is_visible().unwrap_or(false) {
            let _ = win.hide();
            dock.applied.remove(win.label());
        }
    }
}

/// Smooth over flaky reads. Explorer and UI Automation sometimes return no
/// taskbar, or no tray buttons, for a single pass (Explorer busy, taskbar
/// animating, UIA timeout). Without this the pill jumped to the work-area
/// corner above the taskbar, or over the tray icons, until the next good pass.
fn merge_bars(old: &[Bar], new: Vec<Bar>, auto_hide: bool) -> Vec<Bar> {
    if new.is_empty() && !auto_hide {
        return old.to_vec();
    }
    new.into_iter()
        .map(|mut b| {
            if b.limit >= b.bar.right() {
                if let Some(o) = old.iter().find(|o| o.hwnd == b.hwnd && o.bar == b.bar && o.limit < o.bar.right()) {
                    b.limit = o.limit;
                }
            }
            b
        })
        .collect()
}

/// Start the dock thread; it creates the pills on its first pass.
pub fn build_pill(app: &AppHandle) -> tauri::Result<()> {
    if let Ok(mut g) = DOCK.lock() {
        *g = Some(Dock { size: (PILL_W, PILL_H), bars: Vec::new(), applied: Default::default() });
    }
    let app = app.clone();
    std::thread::spawn(move || {
        #[cfg(windows)]
        let uia = uiautomation::UIAutomation::new().ok();
        #[cfg(not(windows))]
        let uia: Option<()> = None;
        loop {
            // A failure in one pass (UIA, Explorer restarting, a window
            // vanishing) must never take the app down: skip it and retry.
            let pass = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let bars = taskbar::bars(uia.as_ref());
                if let Ok(mut g) = DOCK.lock() {
                    if let Some(d) = g.as_mut() {
                        d.bars = merge_bars(&d.bars, bars, taskbar::auto_hide());
                    }
                }
                apply(&app);
            }));
            if pass.is_err() {
                DOCK.clear_poison();
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    Ok(())
}

/// The pill is as wide as its text: the webview measures it and asks for the
/// matching window size; every pill follows.
pub fn fit_pill(app: &AppHandle, width: f64, height: f64) {
    let size = (width.clamp(72.0, 400.0), height.clamp(14.0, 44.0));
    let app = app.clone();
    // Never block the main thread on the dock lock.
    std::thread::spawn(move || {
        if let Ok(mut g) = DOCK.lock() {
            match g.as_mut() {
                Some(d) if d.size != size => d.size = size,
                _ => return,
            }
        }
        apply(&app);
    });
}

fn show_pill(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || apply(&app));
}

fn hide_pill(app: &AppHandle) {
    for win in pills(app) {
        let _ = win.hide();
    }
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
    hide_pill(app);
    set(&state.shown_at, Some(Instant::now()));
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit("window-shown", ());
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
        show_pill(app);
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
    fn pill_sits_in_the_bottom_right_corner_of_the_work_area() {
        let (x, y) = corner(WORK_BOTTOM_TASKBAR, 164, 22);
        assert_eq!(x, 1920 - 164 - PILL_MARGIN);
        assert_eq!(y, 1032 - 22 - PILL_MARGIN);
    }

    #[test]
    fn pill_never_leaves_a_tiny_work_area() {
        assert_eq!(corner(Rect::new(0, 0, 100, 30), 164, 22), (PILL_MARGIN, PILL_MARGIN));
    }

    #[test]
    fn pill_docks_left_of_the_tray_buttons() {
        let bar = Rect::new(0, 1048, 1920, 32);
        let (x, y) = taskbar_slot(bar, 1642, 146, 22).expect("bottom taskbar is dockable");
        assert_eq!(x, 1642 - PILL_MARGIN / 2 - 146);
        assert_eq!(y, 1048 + (32 - 22) / 2);
    }

    #[test]
    fn secondary_bar_docks_left_of_the_clock() {
        let bar = Rect::new(-1920, 832, 1536, 32);
        let (x, _) = taskbar_slot(bar, -465, 146, 22).expect("room left of the clock");
        assert_eq!(x, -465 - PILL_MARGIN / 2 - 146);
    }

    #[test]
    fn vertical_or_tiny_taskbars_fall_back_to_the_corner() {
        assert!(taskbar_slot(Rect::new(1872, 0, 48, 1080), 1080, 164, 22).is_none());
        assert!(taskbar_slot(Rect::new(0, 1074, 1920, 6), 1920, 164, 22).is_none());
        assert!(taskbar_slot(Rect::new(0, 1032, 100, 48), 100, 164, 22).is_none());
    }

    #[test]
    fn pill_labels_round_trip() {
        assert_eq!(pill_index(&pill_label(0)), Some(0));
        assert_eq!(pill_index(&pill_label(2)), Some(2));
        assert_eq!(pill_index("main"), None);
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
