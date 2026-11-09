import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { AccountCard } from "./components/AccountCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { Glyph, IconButton } from "./components/ui";
import { api, on } from "./lib/api";
import { fmtAgo } from "./lib/format";
import { useAppState, useFitWindow, useNow } from "./lib/hooks";
import type { Settings } from "./lib/types";

const GEAR = "\uf013";
const REFRESH = "\uf021";

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
    <div className="dots p-2.5">
      <div
        ref={panelRef}
        className="frame scanlines relative overflow-hidden rounded-[2px] border border-line bg-panel shadow-panel"
      >
        <header className="relative z-20 flex items-center gap-2 border-b border-line px-3 py-[7px]">
          <span className="text-[13px] font-bold leading-none text-accent">❯</span>
          <h1 className="text-[11px] font-extrabold uppercase tracking-[0.24em] text-ink">
            claude<span className="text-accent">·</span>usage
          </h1>
          <span className="cursor mb-px" />
          <div className="ml-auto flex items-center gap-1.5">
            <span className="num flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.16em] text-mute">
              <span
                className={`size-[5px] ${state?.syncing ? "bg-warn pulse-dot" : "bg-ok"}`}
                style={{ boxShadow: state?.syncing ? "0 0 8px rgba(216,166,87,.8)" : "0 0 8px rgba(169,182,101,.8)" }}
              />
              {state?.syncing ? "sync" : "live"}
            </span>
            <IconButton label="Configuración" onClick={() => setView(view === "home" ? "settings" : "home")}>
              <Glyph code={GEAR} className="text-[12px]" />
            </IconButton>
          </div>
        </header>

        {!state ? (
          <div className="h-40" />
        ) : (
          <AnimatePresence mode="wait" initial={false}>
            {view === "settings" ? (
              <div className="relative z-20 px-2.5 py-2.5">
                <SettingsPanel
                  key="settings"
                  settings={state.settings}
                  onChange={saveSettings}
                  onBack={() => setView("home")}
                  onQuit={() => void api.quit()}
                />
              </div>
            ) : (
              <motion.div
                key="home"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.14 }}
                className="relative z-20"
              >
                <div className="max-h-[540px] space-y-2 overflow-y-auto px-2.5 py-2.5">
                  {cards.map(({ card }) => card)}
                </div>
                <footer className="flex items-center justify-between border-t border-line px-3 py-1.5">
                  <span className="num flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.16em] text-mute">
                    <span className="text-accent/60">::</span>
                    {state.last_sync_at ? `sync ${fmtAgo(state.last_sync_at, nowMs)}` : "sin datos"}
                  </span>
                  <IconButton label="Actualizar" onClick={() => void api.refresh(false)} disabled={state.syncing}>
                    <Glyph code={REFRESH} className={`text-[11px] ${state.syncing ? "animate-spin" : ""}`} />
                  </IconButton>
                </footer>
              </motion.div>
            )}
          </AnimatePresence>
        )}

        {notice && (
          <p className="relative z-20 mx-2.5 mb-2.5 border border-bad/40 bg-bad/10 px-2.5 py-1.5 text-[10.5px] text-bad">
            <span className="font-bold">err:</span> {notice}
          </p>
        )}
      </div>
    </div>
  );
}
