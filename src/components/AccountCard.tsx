import { motion } from "motion/react";
import { useState } from "react";
import { fmtAgo, fmtClock, fmtPlan, fmtRemaining, pct } from "../lib/format";
import type { AccountView, LimitWindow } from "../lib/types";
import { Bar, Button, Eyebrow, Glyph, IconButton } from "./ui";

interface Props {
  account: AccountView;
  nowMs: number;
  claudeFound: boolean;
  busy: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onProbe: () => void;
}

const USER = "\uf007";
const MENU = "···";
const LOGIN = "\uf090";
const LOGOUT = "\uf08b";
const SYNC = "\uf021";
const WARN = "\uf071";
const SHIELD = "\uf132";
const UNPLUG = "\uf127";

const tone = (p: number) => (p >= 100 ? "bad" : p >= 80 ? "warn" : "ok");
const toneClass = { ok: "text-ok", warn: "text-warn", bad: "text-bad" } as const;
const toneGlow = {
  ok: "0 0 24px rgba(169,182,101,0.35)",
  warn: "0 0 24px rgba(216,166,87,0.4)",
  bad: "0 0 24px rgba(234,105,98,0.45)",
} as const;

function Avatar({ email }: { email: string | null }) {
  return (
    <div
      className={`grid size-8 shrink-0 place-items-center border text-[13px] font-extrabold ${
        email ? "border-accent/40 bg-accent/10 text-accent" : "border-line bg-panel text-mute"
      }`}
    >
      {email ? email.charAt(0).toUpperCase() : <Glyph code={USER} className="text-[12px]" />}
    </div>
  );
}

function StatusChip({ account }: { account: AccountView }) {
  const map = {
    available: ["ok", "disponible"],
    near_limit: ["warn", "cerca"],
    exhausted: ["bad", "agotado"],
    no_data: ["off", "sin datos"],
    disconnected: ["off", "offline"],
  } as const;
  const [toneKey, text] =
    account.connection === "expired"
      ? (["warn", "expirada"] as const)
      : account.connection === "connecting"
        ? (["off", "conectando"] as const)
        : map[account.status];
  return (
    <span className={`chip tone-${toneKey}`}>
      <span className="size-[3px] bg-current" />
      {text}
    </span>
  );
}

function Window5h({ w, nowMs }: { w: LimitWindow; nowMs: number }) {
  const left = Math.max(0, 100 - w.used_pct);
  const secs = w.resets_at - nowMs / 1000;
  const t = tone(w.used_pct);
  return (
    <div className="space-y-2">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <Eyebrow>ventana 5h</Eyebrow>
          <div className={`figure num mt-1.5 ${toneClass[t]}`} style={{ textShadow: toneGlow[t] }}>
            {pct(w.used_pct)}
            <span className="ml-1 align-baseline text-[9px] font-bold uppercase tracking-[0.16em] text-mute">usado</span>
          </div>
        </div>
        <div className="shrink-0 pt-0.5 text-right text-[10px] leading-relaxed">
          <div className="num text-mute">{pct(left)} libre</div>
          <div className="num flex items-center justify-end gap-1 text-ink">
            <Glyph code={SYNC} className="text-[9px] text-accent" />
            {fmtRemaining(secs)}
          </div>
        </div>
      </div>
      <Bar value={w.used_pct} tone={t} />
      <div className="flex items-center justify-between text-[9px] font-bold uppercase tracking-[0.14em] text-mute">
        <span>reset {fmtClock(w.resets_at, nowMs)}</span>
        <span className="num text-mute/70">{Math.round(w.used_pct)} / 100</span>
      </div>
    </div>
  );
}

function WeeklyRow({ w, nowMs }: { w: LimitWindow; nowMs: number }) {
  const t = tone(w.used_pct);
  return (
    <div className="space-y-1.5 border-t border-line pt-2">
      <div className="flex items-center justify-between gap-2">
        <Eyebrow>semanal · 7d</Eyebrow>
        <span className="num text-[10px]">
          <span className={`font-bold ${toneClass[t]}`}>{pct(w.used_pct)}</span>
          <span className="text-mute"> · {fmtRemaining(w.resets_at - nowMs / 1000)}</span>
        </span>
      </div>
      <Bar value={w.used_pct} tone={t} thin />
    </div>
  );
}

function Body({ account, nowMs, busy, onConnect, onProbe, claudeFound }: Props) {
  const { connection, usage } = account;

  if (connection === "connecting") {
    return (
      <div className="flex items-center gap-2 border border-line bg-faint px-2.5 py-2 text-[10.5px] text-mute">
        <Glyph code={SYNC} className="animate-spin text-accent" />
        esperando el login en la ventana de claude…
      </div>
    );
  }
  if (connection === "checking") {
    return <div className="py-2 text-[10.5px] text-mute">verificando sesión…</div>;
  }
  if (connection !== "connected") {
    const expired = connection === "expired";
    return (
      <div className="space-y-2">
        {(expired || account.error) && (
          <div className="flex items-start gap-1.5 border-l-2 border-warn bg-warn/8 px-2 py-1.5 text-[10.5px] leading-snug text-warn">
            <Glyph code={SHIELD} className="mt-px text-[10px]" />
            <span>{account.error ?? "La sesión expiró."}</span>
          </div>
        )}
        <Button variant="primary" className="w-full" onClick={onConnect} disabled={!claudeFound}>
          <Glyph code={LOGIN} className="text-[10px]" />
          {expired ? "reconectar claude" : "conectar claude"}
        </Button>
        {!claudeFound && (
          <p className="text-[10px] leading-snug text-mute">
            requiere claude code instalado (claude.ai/download). el login lo hace claude code, no esta app.
          </p>
        )}
      </div>
    );
  }

  const five = usage?.five_hour ?? null;
  const week = usage?.seven_day ?? null;
  return (
    <div className="space-y-2.5">
      {five ? (
        <Window5h w={five} nowMs={nowMs} />
      ) : week ? (
        <div className="flex items-center justify-between border border-dashed border-line bg-faint px-2.5 py-1.5 text-[10px] text-mute">
          <span className="num font-bold uppercase tracking-[0.18em]">
            {usage?.five_hour_resets_at
              ? `:: 5h · activa · reinicia ${fmtRemaining(usage.five_hour_resets_at - nowMs / 1000)}`
              : ":: 5h · sin ventana activa"}
          </span>
          <button onClick={onProbe} disabled={busy} className="text-accent disabled:opacity-50" title="consultar ahora">
            <Glyph code={SYNC} className={`text-[10px] ${busy ? "animate-spin" : ""}`} />
          </button>
        </div>
      ) : (
        <div className="space-y-2 border border-dashed border-line bg-faint px-2.5 py-2">
          <div className="flex items-center gap-1.5 text-[9.5px] font-bold uppercase tracking-[0.18em] text-warn">
            <Glyph code={WARN} className="text-[10px]" />
            sin datos
          </div>
          <p className="text-[10px] leading-snug text-mute">
            claude solo informa el uso mientras usas claude code con esta cuenta. úsalo una vez o consulta ahora (consume una
            petición mínima).
          </p>
          <Button onClick={onProbe} disabled={busy}>
            <Glyph code={SYNC} className={`text-[10px] ${busy ? "animate-spin" : ""}`} />
            consultar ahora
          </Button>
        </div>
      )}
      {week && <WeeklyRow w={week} nowMs={nowMs} />}
      {account.error && <p className="text-[10px] text-warn">{account.error}</p>}
      {usage && (
        <p className="num flex items-center justify-between text-[9px] font-bold uppercase tracking-[0.14em] text-mute/70">
          <span>src {usage.source === "probe" ? "probe" : "statusline"}</span>
          <span>{fmtAgo(usage.captured_at, nowMs)}</span>
        </p>
      )}
    </div>
  );
}

export function AccountCard(props: Props) {
  const { account, onDisconnect, onConnect } = props;
  const [menu, setMenu] = useState(false);
  const connected = account.connection === "connected";
  const plan = fmtPlan(account.plan);
  const rail =
    account.connection !== "connected"
      ? "bg-line-hi"
      : account.status === "exhausted"
        ? "bg-bad"
        : account.status === "near_limit"
          ? "bg-warn"
          : "bg-ok";

  return (
    <motion.section
      layout
      initial={{ opacity: 0, y: 5 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", stiffness: 420, damping: 34, delay: account.slot * 0.04 }}
      className="relative border border-line bg-card"
    >
      <span
        className={`absolute inset-y-0 left-0 w-[2px] ${rail}`}
        style={{ boxShadow: connected ? "0 0 12px currentColor" : "none" }}
      />
      <div className="px-3 py-2.5 pl-3.5">
        <header className="mb-2.5 flex items-center gap-2">
          <Avatar email={account.email} />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[12px] font-bold tracking-tight">
              {connected || account.email ? (account.email ?? account.label) : `cuenta ${account.slot} · ${account.label}`}
            </div>
            <div className="mt-0.5 flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.16em] text-mute">
              <span>{account.label}</span>
              {plan && connected && (
                <>
                  <span className="text-line-hi">/</span>
                  <span>{plan}</span>
                </>
              )}
            </div>
          </div>
          <StatusChip account={account} />
          {(connected || account.connection === "expired") && (
            <div className="relative">
              <IconButton
                label="Opciones de cuenta"
                onClick={() => setMenu((v) => !v)}
                onBlur={() => setTimeout(() => setMenu(false), 120)}
              >
                <Glyph code={MENU} className="text-[12px]" />
              </IconButton>
              {menu && (
                <div className="absolute right-0 top-7 z-30 w-44 border border-line-hi bg-panel p-1 shadow-lg">
                  <button
                    className="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[10.5px] uppercase tracking-[0.1em] hover:bg-accent hover:text-on-accent"
                    onMouseDown={onConnect}
                  >
                    <Glyph code={SYNC} className="text-[10px]" /> re-autenticar
                  </button>
                  <button
                    className="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[10.5px] uppercase tracking-[0.1em] text-bad hover:bg-bad hover:text-on-accent"
                    onMouseDown={onDisconnect}
                  >
                    <Glyph code={LOGOUT} className="text-[10px]" /> desconectar
                  </button>
                </div>
              )}
            </div>
          )}
          {account.connection === "error" && <Glyph code={UNPLUG} className="text-[11px] text-bad" />}
        </header>
        <Body {...props} />
      </div>
    </motion.section>
  );
}
