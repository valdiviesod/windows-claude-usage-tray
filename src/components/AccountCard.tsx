import { AlertTriangle, Ellipsis, LogIn, LogOut, RotateCw, ShieldAlert, Unplug, UserRound } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { fmtAgo, fmtClock, fmtPlan, fmtRemaining, pct } from "../lib/format";
import type { AccountView, LimitWindow } from "../lib/types";
import { Bar, Button, IconButton } from "./ui";

interface Props {
  account: AccountView;
  nowMs: number;
  claudeFound: boolean;
  busy: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onProbe: () => void;
}

const tone = (p: number) => (p >= 100 ? "bad" : p >= 80 ? "warn" : "ok");

function Avatar({ email }: { email: string | null }) {
  return (
    <div
      className={`grid size-9 shrink-0 place-items-center rounded-full font-display text-[15px] font-semibold ${
        email ? "bg-gradient-to-br from-accent/90 to-accent/50 text-on-accent" : "bg-faint text-mute"
      }`}
    >
      {email ? email.charAt(0).toUpperCase() : <UserRound className="size-[18px]" />}
    </div>
  );
}

function StatusChip({ account }: { account: AccountView }) {
  const map = {
    available: ["Disponible", "text-ok bg-ok/12"],
    near_limit: ["Cerca del límite", "text-warn bg-warn/15"],
    exhausted: ["Agotado", "text-bad bg-bad/12"],
    no_data: ["Sin datos", "text-mute bg-faint"],
    disconnected: ["Desconectada", "text-mute bg-faint"],
  } as const;
  const [text, cls] =
    account.connection === "expired"
      ? (["Sesión expirada", "text-warn bg-warn/15"] as const)
      : account.connection === "connecting"
        ? (["Conectando…", "text-mute bg-faint"] as const)
        : map[account.status];
  return <span className={`rounded-full px-2 py-px text-[10.5px] font-medium ${cls}`}>{text}</span>;
}

function Window5h({ w, nowMs }: { w: LimitWindow; nowMs: number }) {
  const left = Math.max(0, 100 - w.used_pct);
  const secs = w.resets_at - nowMs / 1000;
  return (
    <div className="space-y-2">
      <div className="flex items-end justify-between">
        <div>
          <div className="text-[10.5px] font-semibold uppercase tracking-wider text-mute">Ventana de 5 horas</div>
          <div className="num font-display text-[28px] font-semibold leading-none tracking-tight">{pct(w.used_pct)}</div>
        </div>
        <div className="num pb-0.5 text-right text-[12px] text-mute">
          <div>{pct(left)} restante</div>
          <div className="text-ink">Reinicia en {fmtRemaining(secs)}</div>
        </div>
      </div>
      <Bar value={w.used_pct} tone={tone(w.used_pct)} />
      <div className="num text-[11.5px] text-mute">Se reinicia a las {fmtClock(w.resets_at, nowMs)}</div>
    </div>
  );
}

function WeeklyRow({ w, nowMs }: { w: LimitWindow; nowMs: number }) {
  return (
    <div className="space-y-1.5 border-t border-line pt-2.5">
      <div className="flex items-center justify-between text-[12px]">
        <span className="text-mute">Semanal</span>
        <span className="num">
          <span className="font-medium">{pct(w.used_pct)}</span>
          <span className="text-mute"> · reinicia en {fmtRemaining(w.resets_at - nowMs / 1000)}</span>
        </span>
      </div>
      <Bar value={w.used_pct} tone={tone(w.used_pct)} thin />
    </div>
  );
}

function Body({ account, nowMs, busy, onConnect, onProbe, claudeFound }: Props) {
  const { connection, usage } = account;

  if (connection === "connecting") {
    return (
      <div className="flex items-center gap-2.5 rounded-xl bg-faint px-3 py-3 text-[12.5px] text-mute">
        <RotateCw className="size-4 animate-spin" />
        Completa el inicio de sesión en la ventana que se abrió…
      </div>
    );
  }
  if (connection === "checking") {
    return <div className="py-3 text-[12.5px] text-mute">Verificando sesión…</div>;
  }
  if (connection !== "connected") {
    const expired = connection === "expired";
    return (
      <div className="space-y-3">
        {(expired || account.error) && (
          <div className="flex items-start gap-2 rounded-xl bg-warn/12 px-3 py-2 text-[12px] text-warn">
            <ShieldAlert className="mt-px size-3.5 shrink-0" />
            <span>{account.error ?? "La sesión expiró."}</span>
          </div>
        )}
        <Button variant="primary" className="w-full" onClick={onConnect} disabled={!claudeFound}>
          <LogIn className="size-3.5" />
          {expired ? "Reconectar Claude" : "Conectar Claude"}
        </Button>
        {!claudeFound && (
          <p className="text-[11.5px] text-mute">
            Requiere Claude Code instalado (claude.ai/download). El inicio de sesión lo hace Claude Code, no esta app.
          </p>
        )}
      </div>
    );
  }

  const five = usage?.five_hour ?? null;
  const week = usage?.seven_day ?? null;
  return (
    <div className="space-y-3">
      {five ? (
        <Window5h w={five} nowMs={nowMs} />
      ) : (
        <div className="space-y-2.5 rounded-xl bg-faint px-3 py-3">
          <div className="flex items-center gap-2 text-[13px] font-medium">
            <AlertTriangle className="size-3.5 text-mute" />
            No disponible
          </div>
          <p className="text-[12px] leading-snug text-mute">
            Claude solo informa el uso mientras se usa Claude Code con esta cuenta. Úsalo una vez o consulta ahora (consume una
            petición mínima).
          </p>
          <Button onClick={onProbe} disabled={busy}>
            <RotateCw className={`size-3.5 ${busy ? "animate-spin" : ""}`} />
            Consultar ahora
          </Button>
        </div>
      )}
      {week && <WeeklyRow w={week} nowMs={nowMs} />}
      {account.error && <p className="text-[11.5px] text-warn">{account.error}</p>}
      {usage && (
        <p className="num text-[11px] text-mute/80">
          Dato de {fmtAgo(usage.captured_at, nowMs)} · {usage.source === "probe" ? "consulta directa" : "Claude Code"}
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

  return (
    <motion.section
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", stiffness: 380, damping: 32, delay: account.slot * 0.04 }}
      className="relative rounded-xl bg-card p-3.5 shadow-card"
    >
      <header className="mb-3 flex items-center gap-2.5">
        <Avatar email={account.email} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-[13.5px] font-semibold">
              {connected || account.email ? (account.email ?? account.label) : `Cuenta ${account.slot} · ${account.label}`}
            </span>
          </div>
          <div className="flex items-center gap-1.5 text-[11.5px] text-mute">
            <span>{account.label}</span>
            {plan && connected && (
              <>
                <span>·</span>
                <span>{plan}</span>
              </>
            )}
          </div>
        </div>
        <StatusChip account={account} />
        {(connected || account.connection === "expired") && (
          <div className="relative">
            <IconButton label="Opciones de cuenta" onClick={() => setMenu((v) => !v)} onBlur={() => setTimeout(() => setMenu(false), 120)}>
              <Ellipsis className="size-4" />
            </IconButton>
            {menu && (
              <div className="absolute right-0 top-8 z-10 w-44 overflow-hidden rounded-lg border border-line bg-panel p-1 shadow-lg backdrop-blur-xl">
                <button className="flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-[12.5px] hover:bg-accent hover:text-on-accent" onMouseDown={onConnect}>
                  <RotateCw className="size-3.5" /> Volver a autenticar
                </button>
                <button className="flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-[12.5px] text-bad hover:bg-accent hover:text-on-accent" onMouseDown={onDisconnect}>
                  <LogOut className="size-3.5" /> Desconectar
                </button>
              </div>
            )}
          </div>
        )}
        {account.connection === "error" && <Unplug className="size-4 text-bad" />}
      </header>
      <Body {...props} />
    </motion.section>
  );
}
