# Claude Usage Tray

Windows 11 tray utility (Tauri 2 + React + TypeScript + Tailwind + Rust) that shows the
real usage limits of **two independent Claude Pro/Max accounts** without opening a browser.

- One tray icon. Left click toggles the floating window; right click: *Actualizar*,
  *Configuración*, *Salir*. Tooltip summarises both accounts.
- Background sync runs in Rust (works with the window hidden), 60 s by default, with
  cache, manual refresh, exponential backoff and single-flight syncs.
- Window hides on blur; closing it never stops the process. Single instance. Starts with
  Windows (hidden). Optional notifications at 80 / 95 / 100 %.
- Idle cost: one small Rust process (~25 MB) + WebView2 that is not rendering while hidden.

> **Read first:** [docs/RESEARCH.md](docs/RESEARCH.md). Anthropic does not allow third-party
> apps to collect Claude credentials, so this app has **no OAuth/password flow of its own**.
> Sign-in is done by the official `claude` CLI, and usage comes from the limits Claude Code
> itself reports. Consequence: figures appear once the account has been used in Claude Code
> (or after pressing *Consultar ahora*). If a value is unknown the UI says "No disponible".

## Requirements

- Windows 11 with WebView2 (already included).
- [Claude Code](https://claude.ai/download) installed (`claude` on PATH or in `%USERPROFILE%\.local\bin`).
- Two Claude Pro/Max accounts (or one: the other card just stays disconnected).

## Install

Run `src-tauri\target\release\bundle\nsis\Claude Usage_0.2.1_x64-setup.exe` (per-user, no admin)
or the `.msi` next to it. The app starts hidden in the tray area (click `^` to find it and
drag it onto the taskbar if you want it always visible).

## Use

1. Click the tray icon. Each card says **Conectar Claude**.
2. Press it for account 1: a console opens running the official `claude auth login`; the
   browser opens Claude's own sign-in. Pick the account you want as *Personal*. Repeat for
   account 2 (*Trabajo*) with the other account (use a private window or switch account on
   the claude.ai page if the browser is already signed in to the first one).
3. Each account lives in its own isolated Claude Code profile:
   `%APPDATA%\com.claudeusage.tray\accounts\<1|2>\claude`. The app configures that profile's
   `statusLine` to call `claude-usage-tray.exe --statusline <n>` (a 50 ms helper that saves
   the limits Claude Code reports).
4. To feed an account with data, use Claude Code with that profile, e.g. in PowerShell:

   ```powershell
   $env:CLAUDE_CONFIG_DIR = "$env:APPDATA\com.claudeusage.tray\accounts\1\claude"; claude
   ```

   or press **Consultar ahora** in the card (one minimal request that consumes a little quota),
   or enable *Consulta directa automática* in settings (default off, min 10 min).
5. **⋯ menu** on a card: *Volver a autenticar*, *Desconectar* (runs `claude auth logout` for that
   profile only and deletes its snapshot).

## Build from source

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
npm install
npm run tauri build      # installers in src-tauri\target\release\bundle
npm run tauri dev        # development
cd src-tauri; cargo test --lib
```

## Architecture

```
src-tauri/src
  model.rs      shared types, status computation (available / near limit / exhausted / no data)
  paths.rs      per-account directory layout (no shared paths)
  snapshot.rs   statusline JSON parser + `--statusline` helper entry point
  claude_cli.rs wrapper over the official `claude` binary (auth status/login/logout/probe)
  provider.rs   UsageProvider trait: StatuslineProvider (free) / ProbeProvider (opt-in)
  sync.rs       background engine: per-account state machine, backoff, alerts, tray tooltip
  cache.rs      last-known data across restarts (never shown as live)
  alerts.rs     80/95/100 % thresholds (once per window), backoff and time formatting
  window.rs     placement above the tray icon (any taskbar edge/monitor), toggle, hide-on-blur
  tray.rs       single tray icon + menu
src/            React UI: no polling, state pushed by Rust events; timers only while visible
```

Security: no secrets stored by the app; `ANTHROPIC_API_KEY` & co. are scrubbed from every
child process so they cannot override a profile; the statusline helper refuses to write if
`CLAUDE_CONFIG_DIR` points at another account's profile.

## Tests

`cargo test --lib` (41 tests): account isolation (paths, snapshots, statusline helper),
parsing and corrupt data, status thresholds, expired windows, auth state machine
(login / logout / expiry / identity change / restart), error backoff, notifications once per
window, cache restart roundtrip, window placement (bottom/top/left/right taskbar, multi
monitor, negative origins) and the tray-click-after-blur guard.

Connected accounts appear first, followed by accounts without an active session.
Within each group, the original slot order is preserved.
The order updates automatically when connection state changes.
