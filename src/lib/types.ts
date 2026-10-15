export type Connection = "disconnected" | "checking" | "connecting" | "connected" | "expired" | "error";
export type UsageStatus = "available" | "near_limit" | "exhausted" | "no_data" | "disconnected";

export interface LimitWindow {
  used_pct: number; // 0..100
  resets_at: number; // epoch seconds
}

export interface UsageSnapshot {
  captured_at: number; // epoch seconds
  source: "statusline" | "probe" | string;
  five_hour: LimitWindow | null;
  seven_day: LimitWindow | null;
}

export interface AccountView {
  slot: 1 | 2;
  label: string;
  connection: Connection;
  email: string | null;
  plan: string | null;
  usage: UsageSnapshot | null;
  status: UsageStatus;
  error: string | null;
}

export interface Settings {
  interval_secs: number;
  notifications: boolean;
  autostart: boolean;
  probe_enabled: boolean;
  probe_interval_mins: number;
}

export interface AppView {
  now: number;
  accounts: AccountView[];
  settings: Settings;
  syncing: boolean;
  last_sync_at: number | null;
  claude_found: boolean;
}
