/**
 * Usage readout docked in the Windows taskbar (native window label `pill`).
 * It renders whatever the engine pushes, measures itself and asks the native
 * window to match that size. Placement, transparency and the z-order guard
 * live in Rust: see docs/TRAY-PILL.md.
 */
import { useEffect, useRef } from "react";
import { api } from "../lib/api";
import { useAppState } from "../lib/hooks";
import type { AccountView } from "../lib/types";

type Tone = "ok" | "warn" | "bad" | "mute";

const RANK: Record<Tone, number> = { ok: 0, mute: 1, warn: 2, bad: 3 };

const TONE_TEXT: Record<Tone, string> = {
  ok: "text-ok",
  warn: "text-warn",
  bad: "text-bad",
  mute: "text-mute",
};

const TONE_BG: Record<Tone, string> = {
  ok: "bg-ok",
  warn: "bg-warn",
  bad: "bg-bad",
  mute: "bg-mute",
};

const TONE_GLOW: Record<Tone, string> = {
  ok: "0 0 8px rgba(169,182,101,0.85)",
  warn: "0 0 8px rgba(216,166,87,0.85)",
  bad: "0 0 8px rgba(234,105,98,0.9)",
  mute: "none",
};

const toneOf = (pct: number | null): Tone => (pct === null ? "mute" : pct >= 100 ? "bad" : pct >= 80 ? "warn" : "ok");

const pctOf = (account: AccountView, window: "five_hour" | "seven_day"): number | null =>
  account.usage?.[window]?.used_pct ?? null;

function Value({ value }: { value: number | null }) {
  return (
    <span className={`num text-[10.5px] font-bold leading-none ${TONE_TEXT[toneOf(value)]}`}>
      {value === null ? "--" : `${Math.round(value)}%`}
    </span>
  );
}

function Label({ children }: { children: string }) {
  return <span className="text-[8.5px] font-bold uppercase leading-none tracking-[0.16em] text-mute">{children}</span>;
}

function Dot() {
  return <span className="text-[9px] leading-none text-line-hi">·</span>;
}

/** One account: 5h 16% · week 24% */
function Single({ account }: { account: AccountView }) {
  return (
    <span className="flex items-center gap-1">
      <Label>5h</Label>
      <Value value={pctOf(account, "five_hour")} />
      <Dot />
      <Label>week</Label>
      <Value value={pctOf(account, "seven_day")} />
    </span>
  );
}

/** Two accounts: p 16%/24% · t 5%/8% */
function Compact({ account }: { account: AccountView }) {
  return (
    <span className="flex items-center gap-1">
      <span className="text-[9px] font-extrabold uppercase leading-none tracking-[0.1em] text-accent">
        {account.slot === 1 ? "p" : "t"}
      </span>
      <Value value={pctOf(account, "five_hour")} />
      <span className="text-[9px] leading-none text-line-hi">/</span>
      <Value value={pctOf(account, "seven_day")} />
    </span>
  );
}

export function UsagePill() {
  const [state, setState] = useAppState();
  const ref = useRef<HTMLButtonElement>(null);

  const connected = (state?.accounts ?? []).filter((a) => a.connection === "connected");
  const tone = connected.reduce<Tone>(
    (acc, a) => {
      const t = toneOf(pctOf(a, "five_hour"));
      return RANK[t] > RANK[acc] ? t : acc;
    },
    connected.length === 0 ? "mute" : "ok",
  );

  // The native window is exactly as wide as this button.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let last = "";
    let raf = 0;
    const fit = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        const w = Math.ceil(r.width);
        const h = Math.ceil(r.height);
        if (w < 2 || h < 2) return;
        const key = `${w}x${h}`;
        if (key === last) return;
        last = key;
        void api.fitPill(w, h);
      });
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    fit();
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, []);

  // A hidden webview can miss events: refresh whenever it comes back.
  useEffect(() => {
    const sync = () => {
      if (document.visibilityState === "visible") void api.getState().then(setState);
    };
    document.addEventListener("visibilitychange", sync);
    return () => document.removeEventListener("visibilitychange", sync);
  }, [setState]);

  return (
    <div className="flex w-fit">
      <button
        ref={ref}
        onClick={() => void api.toggleWindow()}
        aria-label="Uso de Claude — abrir panel"
        title="Claude Usage"
        className="flex h-[22px] items-center gap-1.5 border border-line bg-panel px-2 text-ink transition-colors hover:border-line-hi hover:bg-card"
      >
        <span className={`size-[4px] shrink-0 ${TONE_BG[tone]}`} style={{ boxShadow: TONE_GLOW[tone] }} />
        {connected.length === 0 ? (
          <span className="text-[8.5px] font-bold uppercase leading-none tracking-[0.16em] text-mute">
            claude<span className="text-accent/60"> · </span>offline
          </span>
        ) : connected.length === 1 ? (
          <Single account={connected[0]} />
        ) : (
          <span className="flex items-center gap-1.5">
            {connected.map((a, i) => (
              <span key={a.slot} className="flex items-center gap-1.5">
                {i > 0 && <span className="text-[9px] leading-none text-line-hi">·</span>}
                <Compact account={a} />
              </span>
            ))}
          </span>
        )}
      </button>
    </div>
  );
}
