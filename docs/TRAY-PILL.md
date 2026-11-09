# Tray usage pill

The small text readout docked in the Windows taskbar (`5H 24% · WEEK 25%`). This document
explains why it is a separate window, how it is sized, where it is placed, why it needs a
z-order guard, and what it does not do.

Date of last change: 2026-10-31.

> **2026-10-31:** there is now one pill per taskbar (`pill`, `pill-1`, …; primary plus every
> `Shell_SecondaryTrayWnd`). Each one docks immediately left of the first tray button
> (`SystemTray.AccentButton` via UI Automation: the chevron / keyboard language on the
> primary bar, the clock on secondary bars), with `TrayNotifyWnd` as the fallback. A dock
> thread in `window.rs` re-reads the bars every second and only moves a pill when its rect
> changes. Sections below that describe the single pill and `keep_pill_on_top` are historical.

## Why a window and not the tray icon

Windows tray icons are 16×16 logical px at 100 % scaling (24×24 at 150 %). A tray icon can
hold a picture, not a caption, and Tauri's `TrayIcon::set_title` — the API that puts text next
to the icon — is macOS-only. Rendering `5h 16% · week 24%` into a 16 px bitmap is unreadable,
so the text lives in its own borderless, transparent, always-on-top window that is anchored
over the taskbar, immediately left of the notification area (chevron, keyboard language `ENG`,
network/volume/battery, clock).

```
                     taskbar (Shell_TrayWnd)
  ┌───────────────┬──────────────────────┬──────────────────────────────────┐
  │ app buttons   │ ▣ 5H 24% · WEEK 25%  │ ^  ENG  wifi  vol  bat  12:41 PM │
  └───────────────┴──────────────────────┴──────────────────────────────────┘
                    ↑ pill (TrayNotifyWnd.left - margin - width)
```

## What it shows

| State | Text | Notes |
|---|---|---|
| No account connected | `CLAUDE · OFFLINE` | grey dot, no numbers |
| One connected account | `5H 24% · WEEK 25%` | 5-hour window, then weekly |
| Two connected accounts | `P 24%/25% · T 5%/8%` | `P` = slot 1 *Personal*, `T` = slot 2 *Trabajo*, each `5h/week` |
| A window has no data | `--` | unknown is never rendered as `0%` |
| First render / no state yet | `CLAUDE · OFFLINE` | same placeholder |

Colours follow the panel's thresholds: `ok` (`#a9b665`) below 80 %, `warn` (`#d8a657`) from
80 %, `bad` (`#ea6962`) at 100 %. The dot uses the worst tone of the connected accounts and
takes the same glow as the card rails. A percentage is only shown for accounts whose
`connection == "connected"`; disconnected slots are omitted, which is why a single-account
setup shows the long format.

Labels use the app typeface (`JetBrainsMono Nerd Font` → `JetBrains Mono` → monospace) at
8.5 px uppercase with 0.16 em tracking; values are 10.5 px bold with tabular numerals
(`.num`). The markup writes `5h` / `week` / `claude · offline` in lowercase and CSS uppercases
it, so what you read on the taskbar is `5H`, `WEEK`, `CLAUDE · OFFLINE`. Window height is a
fixed 22 px, so the text is vertically centred by flexbox.

## Files

| File | Responsibility |
|---|---|
| `pill.html` | second Vite entry (multi-page build) |
| `src/pill.tsx` | mounts `UsagePill`, installs the dev mock in a plain browser |
| `src/components/UsagePill.tsx` | text formatting, tones, self-sizing, click → `toggle_window` |
| `src/lib/api.ts` | `fitPill(width, height)`, `toggleWindow()` |
| `vite.config.ts` | `build.rollupOptions.input = { main: index.html, pill: pill.html }` |
| `src-tauri/src/lib.rs` | `fit_pill` / `toggle_window` commands, `window::build_pill` in `setup` |
| `src-tauri/src/window.rs` | window creation, placement (`taskbar_slot`, `corner`), z-order guard, show/hide with the panel |
| `src-tauri/capabilities/default.json` | `windows: ["main", "pill"]` (needed for `listen`) |

The pill is **not** declared in `tauri.conf.json`; it is created at runtime by
`WebviewWindowBuilder` so it can be positioned before it is shown.

## Data flow

```
Rust engine ──emit("state-changed", AppView)──┬─> main webview (panel)
                                              └─> pill webview (UsagePill)
UsagePill ──invoke fit_pill(w,h)──> window::fit_pill ──> set_size + place_pill
UsagePill ──invoke toggle_window──> window::toggle (same path as a tray click)
```

No polling: the pill renders whatever the engine pushes. A WebView2 window that was hidden can
miss events, so the component re-reads `get_state` whenever `document.visibilityState` becomes
`visible` (which happens each time the pill is shown again).

## Sizing contract

The webview is the source of truth for its own size:

1. `UsagePill` observes its `<button>` with a `ResizeObserver` (coalesced in
   `requestAnimationFrame`, guarded by a `WxH` cache so a resize loop cannot form).
2. It invokes `fit_pill(width, height)` with CSS pixels.
3. `window::fit_pill` clamps to `72..=400 × 14..=44` logical px, calls `set_size(LogicalSize)`
   and immediately re-anchors with `place_pill` using the size it just requested (querying
   `outer_size` right after `set_size` races with the window manager on Windows).

Because the width follows the text, the pill grows when a second account connects and shrinks
back to `CLAUDE · OFFLINE` when both go offline; it always stays right-anchored.

The same contract drives the panel (`fit_window`, height only); the pill only adds the width.

## Placement

### Docked to the taskbar (normal case)

`taskbar::info()` reads the real geometry with Win32, in physical pixels:

| Call | Purpose |
|---|---|
| `FindWindowW("Shell_TrayWnd")` | main taskbar window |
| `IsWindowVisible` | taskbar exists and is not hidden |
| `SHAppBarMessage(ABM_GETSTATE)` | auto-hide detection (`ABS_AUTOHIDE`) |
| `FindWindowExW(bar, "TrayNotifyWnd")` | notification area (chevron → clock) |
| `GetWindowRect` (both) | rectangles |

`taskbar_slot(bar, notify, w, h)` then computes:

```
x = max(notify.left - PILL_MARGIN - w, bar.left + PILL_MARGIN)
y = bar.top + (bar.height - h) / 2
```

with these guards (anything failing → fallback):

- horizontal taskbar only (`bar.width > bar.height`);
- the taskbar is not set to auto-hide (`SHAppBarMessage(ABM_GETSTATE) & ABS_AUTOHIDE`): an
  auto-hidden bar is parked off-screen, so docking to it would hide the pill;
- the taskbar is at least 24 physical px thick;
- the pill fits vertically (`h <= bar.height - 4`);
- the notification area is inside the taskbar (`notify.left > bar.left`,
  `notify.right <= bar.right + 1`).

`PILL_MARGIN` (8) is in physical pixels, so the visual gap is slightly smaller at high DPI.

### Fallback

`corner(work, w, h)` puts the pill in the bottom-right corner of the monitor's work area, 8 px
from both edges. It is used when the taskbar is vertical, auto-hidden, missing (`info()` is
`None`), or when the notification area is unusable — e.g. under Explorer restarts.

The monitor is the one that owns the tray icon rect when known, otherwise the primary monitor;
the position is clamped so the pill never leaves the work area.

## Z-order: staying above the taskbar

`Shell_TrayWnd` is a topmost window. A click on the taskbar raises it **above** the pill, and a
plain `always_on_top` window does not win that race again on its own.

`taskbar::keep_pill_on_top(hwnd)` spawns one thread that, every 500 ms:

1. walks the z-order upwards from the pill with `GetWindow(GW_HWNDPREV)` (max 512 hops);
2. if it meets `Shell_TrayWnd` before the top of the band, the taskbar is covering the pill;
3. only then calls `SetWindowPos(pill, HWND_TOPMOST, 0,0,0,0, SWP_NOMOVE | SWP_NOSIZE |
   SWP_NOACTIVATE)`.

The conditional raise is what keeps the pill from jumping above other topmost surfaces: if the
Start menu or a flyout is in front, the taskbar is not the window that covers the pill, so
nothing is raised. The badge reappears within half a second of a taskbar click.

Cost: a handful of cheap `USER32` calls on a sleeping thread every 500 ms, and only while the
process is alive.

## Lifecycle with the panel

| Event | Pill |
|---|---|
| App start | built with `visible(false)`, positioned, then shown (no flash at the wrong place) |
| Click on the pill | `toggle_window` → panel shows, pill hides |
| Panel closes (blur, `Esc`, tray click) | pill shows again |
| Panel open | pill hidden — it would otherwise overlap the panel above the tray area |
| Taskbar click | stays visible thanks to the z-order guard |

`window::show` hides the pill, `window::hide` shows it; the panel's own placement is unchanged.

## DPI and monitors

- All geometry from Win32 is physical; the requested size is logical and converted with the
  pill window's `scale_factor()` (at 125 %, 22 logical px → 28 physical px).
- The pill re-anchors on every `fit_pill`, so a DPI change or a taskbar resize is corrected the
  next time the text changes or the panel is closed.
- Only the **primary** taskbar is handled. A secondary-monitor taskbar
  (`Shell_SecondaryTrayWnd`) uses the same fallback as a vertical bar.

## Testing

Automated (`cargo test --lib`, part of the 45 tests):

- `pill_sits_in_the_bottom_right_corner_of_the_work_area` — fallback geometry;
- `pill_never_leaves_a_tiny_work_area` — clamping;
- `pill_docks_left_of_the_notification_area` — `x`/`y` math for a bottom taskbar;
- `vertical_or_autohidden_taskbars_fall_back_to_the_corner` — the three rejection guards.

Manual verification performed on Windows 11 (1920×1080, 125 %):

1. app cold start → pill visible in the taskbar, no panel, no flash;
2. click on the taskbar empty area → pill disappears for < 0.5 s and returns;
3. click on the pill → panel opens, pill hidden (`WindowFromPoint` confirms the click lands on
   `Tauri Window`, not on `Shell_TrayWnd`);
4. click outside the panel → panel closes, pill returns;
5. release build installed through NSIS behaves the same as the dev build.

## Limitations

- No text can be placed *between* tray icons; the pill sits left of the whole notification
  area. Moving it between icons would need a real deskband/toolbar (COM `DeskBand`), which
  requires registration and elevation-level shell integration.
- If the taskbar is set to auto-hide, the pill uses the corner fallback (an auto-hidden bar is
  parked off-screen, so it cannot host the pill).
- Explorer must be running: with the taskbar gone there is nowhere to dock.
- The pill is a second WebView2 with its own renderer process; it only renders while visible
  and is hidden whenever the panel is open.

## Tuning

| Where | Constant | Meaning |
|---|---|---|
| `window.rs` | `PILL_MARGIN` | gap to the notification area / work-area edge (physical px) |
| `window.rs` | `PILL_W`, `PILL_H` | initial size before the first `fit_pill` |
| `window.rs` | `fit_pill` clamps | min/max window size |
| `window.rs` | `keep_pill_on_top` interval | 500 ms z-order check |
| `UsagePill.tsx` | `toneOf` | 80 % / 100 % colour thresholds |
| `UsagePill.tsx` | `Single` / `Compact` | text formats |
| `index.css` | fonts, `--ok` / `--warn` / `--bad` | typeface and palette |
