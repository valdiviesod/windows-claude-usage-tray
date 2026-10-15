// Dev-only: lets the UI be previewed in a plain browser (`npm run dev`).
// Never bundled in production (guarded by import.meta.env.DEV in main.tsx).
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppView } from "./lib/types";

const now = Math.floor(Date.now() / 1000);

const scenarios: Record<string, AppView["accounts"]> = {
  default: [
    {
      slot: 1, label: "Personal", connection: "connected", email: "ana@example.com", plan: "max", error: null, status: "near_limit",
      usage: { captured_at: now - 120, source: "statusline", five_hour: { used_pct: 86, resets_at: now + 4980 }, seven_day: { used_pct: 41, resets_at: now + 3 * 86400 } },
    },
    {
      slot: 2, label: "Trabajo", connection: "connected", email: "ana@empresa.com", plan: "pro", error: null, status: "no_data", usage: null,
    },
  ],
  fresh: [
    { slot: 1, label: "Personal", connection: "disconnected", email: null, plan: null, usage: null, status: "disconnected", error: null },
    { slot: 2, label: "Trabajo", connection: "disconnected", email: null, plan: null, usage: null, status: "disconnected", error: null },
  ],
  mixed: [
    {
      slot: 1, label: "Personal", connection: "connected", email: "ana@example.com", plan: "pro", error: null, status: "available",
      usage: { captured_at: now - 30, source: "statusline", five_hour: { used_pct: 23, resets_at: now + 9000 }, seven_day: null },
    },
    { slot: 2, label: "Trabajo", connection: "expired", email: "ana@empresa.com", plan: "pro", usage: null, status: "disconnected", error: "La sesión expiró. Vuelve a conectar la cuenta." },
  ],
};

export function installDevMock() {
  const key = new URLSearchParams(location.search).get("s") ?? "default";
  const view: AppView = {
    now,
    accounts: scenarios[key] ?? scenarios.default,
    settings: { interval_secs: 60, notifications: true, autostart: true, probe_enabled: false, probe_interval_mins: 30 },
    syncing: false,
    last_sync_at: now - 40,
    claude_found: true,
  };
  mockIPC(
    (cmd, args) => {
      if (cmd === "get_state") return view;
      if (cmd === "save_settings") return (args as { settings: unknown }).settings;
      return null;
    },
    { shouldMockEvents: true },
  );
}
