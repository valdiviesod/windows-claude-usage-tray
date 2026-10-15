import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppView, Settings } from "./types";

export const api = {
  getState: () => invoke<AppView>("get_state"),
  refresh: (probe = false) => invoke<void>("refresh", { probe }),
  connect: (slot: number) => invoke<void>("connect_account", { slot }),
  disconnect: (slot: number) => invoke<void>("disconnect_account", { slot }),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  fitWindow: (height: number) => invoke<void>("fit_window", { height }),
  hide: () => invoke<void>("hide_window"),
  quit: () => invoke<void>("quit_app"),
};

export function on<T = unknown>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
