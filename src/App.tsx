import { Gauge, RefreshCw, Settings as SettingsIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { AccountCard } from "./components/AccountCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { IconButton } from "./components/ui";
import { api, on } from "./lib/api";
import { fmtAgo } from "./lib/format";
import { useAppState, useFitWindow, useNow } from "./lib/hooks";
import type { Settings } from "./lib/types";

export default function App() {
  const [state, setState] = useAppState();
  const nowMs = useNow();
  const panelRef = useFitWindow<HTMLDivElement>();
  const [view, setView] = useState<"home" | "settings">("home");
  const [probing, setProbing] = useState<number | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const off = on("open-settings", () => setView("settings"));
    return () => void off.then((f) => f());
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && (view === "settings" ? setView("home") : api.hide());
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view]);

  const fail = (e: unknown) => {
    setNotice(String(e));
    setTimeout(() => setNotice(null), 6000);
  };

  const saveSettings = (s: Settings) => {
    setState((prev) => (prev ? { ...prev, settings: s } : prev));
    api.saveSettings(s).catch(fail);
  };

  const probe = async (slot: number) => {
    setProbing(slot);
    try {
      await api.refresh(true);
    } finally {
      setProbing(null);
    }
  };

  const cards = state
    ? state.accounts
        .map((a) => ({
          connected: a.connection === "connected",
          card: (
            <AccountCard
              key={`claude-${a.slot}`}
              account={a}
              nowMs={nowMs}
              claudeFound={state.claude_found}
              busy={probing === a.slot || state.syncing}
              onConnect={() => api.connect(a.slot).catch(fail)}
              onDisconnect={() => api.disconnect(a.slot).catch(fail)}
              onProbe={() => void probe(a.slot)}
            />
          ),
        }))
        .sort((x, y) => Number(y.connected) - Number(x.connected))
    : [];

  return (
    <div className="p-3">
      <div ref={panelRef} className="overflow-hidden rounded-[14px] bg-panel p-3 shadow-panel backdrop-blur-2xl">
        <header className="mb-3 flex items-center justify-between px-0.5">
          <div className="flex items-center gap-2">
            <Gauge className="size-4 text-accent" strokeWidth={2.2} />
            <h1 className="font-display text-[13px] font-semibold">Claude Usage</h1>
            <span
              title={state?.syncing ? "Sincronizando…" : "Sincronizado"}
              className={`size-1.5 rounded-full ${state?.syncing ? "animate-pulse bg-warn" : "bg-switch"}`}
            />
          </div>
          <IconButton label="Configuración" onClick={() => setView(view === "home" ? "settings" : "home")}>
            <SettingsIcon className="size-4" />
          </IconButton>
        </header>

        {!state ? (
          <div className="h-40" />
        ) : (
          <AnimatePresence mode="wait" initial={false}>
            {view === "settings" ? (
              <SettingsPanel
                key="settings"
                settings={state.settings}
                onChange={saveSettings}
                onBack={() => setView("home")}
                onQuit={() => void api.quit()}
              />
            ) : (
              <motion.div key="home" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="max-h-[540px] overflow-y-auto space-y-2.5">
                {cards.map(({ card }) => card)}
                <footer className="flex items-center justify-between px-1 pt-1 text-[11.5px] text-mute">
                  <span className="num">
                    {state.last_sync_at ? `Última actualización ${fmtAgo(state.last_sync_at, nowMs)}` : "Sin actualizar"}
                  </span>
                  <div className="flex items-center gap-0.5">
                    <IconButton label="Actualizar" onClick={() => void api.refresh(false)} disabled={state.syncing}>
                      <RefreshCw className={`size-3.5 ${state.syncing ? "animate-spin" : ""}`} />
                    </IconButton>
                  </div>
                </footer>
              </motion.div>
            )}
          </AnimatePresence>
        )}

        {notice && <p className="mt-2 rounded-md bg-bad/12 px-3 py-2 text-[12px] text-bad">{notice}</p>}
      </div>
    </div>
  );
}
