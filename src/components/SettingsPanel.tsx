import { ChevronLeft, Info, Power } from "lucide-react";
import { motion } from "motion/react";
import type { ReactNode } from "react";
import type { Settings } from "../lib/types";
import { Button, IconButton, Toggle } from "./ui";

interface Props {
  settings: Settings;
  onChange: (s: Settings) => void;
  onBack: () => void;
  onQuit: () => void;
}

function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="min-w-0">
        <div className="text-[13px] font-medium">{title}</div>
        {hint && <div className="mt-0.5 text-[11.5px] leading-snug text-mute">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

function Select<T extends number>({ value, options, onChange, label }: { value: T; options: [T, string][]; onChange: (v: T) => void; label: string }) {
  return (
    <select
      aria-label={label}
      value={value}
      onChange={(e) => onChange(Number(e.target.value) as T)}
      className="h-7 rounded-md border border-line bg-card px-2 text-[12.5px] shadow-card outline-none"
    >
      {options.map(([v, text]) => (
        <option key={v} value={v}>
          {text}
        </option>
      ))}
    </select>
  );
}

export function SettingsPanel({ settings, onChange, onBack, onQuit }: Props) {
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => onChange({ ...settings, [k]: v });
  return (
    <motion.div initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} className="space-y-3">
      <div className="flex items-center gap-1">
        <IconButton label="Volver" onClick={onBack}>
          <ChevronLeft className="size-4" />
        </IconButton>
        <h2 className="font-display text-[15px] font-semibold">Configuración</h2>
      </div>

      <div className="divide-y divide-line rounded-xl bg-card px-3.5 shadow-card">
        <Row title="Actualización" hint="Lectura local de los datos de cada cuenta.">
          <Select
            label="Intervalo de actualización"
            value={settings.interval_secs}
            onChange={(v) => set("interval_secs", v)}
            options={[
              [30, "30 s"],
              [60, "60 s"],
              [120, "2 min"],
              [300, "5 min"],
            ]}
          />
        </Row>
        <Row title="Iniciar con Windows" hint="Arranca oculto en la bandeja.">
          <Toggle label="Iniciar con Windows" checked={settings.autostart} onChange={(v) => set("autostart", v)} />
        </Row>
        <Row title="Notificaciones" hint="Al llegar a 80%, 95% y 100%.">
          <Toggle label="Notificaciones" checked={settings.notifications} onChange={(v) => set("notifications", v)} />
        </Row>
        <Row title="Consulta directa automática" hint="Solo cuando Claude Code no ha reportado datos recientes. Petición mínima (consumo medido < 0,1% por consulta).">
          <Toggle label="Consulta directa automática" checked={settings.probe_enabled} onChange={(v) => set("probe_enabled", v)} />
        </Row>
        {settings.probe_enabled && (
          <Row title="Cada">
            <Select
              label="Intervalo de consulta directa"
              value={settings.probe_interval_mins}
              onChange={(v) => set("probe_interval_mins", v)}
              options={[
                [1, "1 min"],
                [2, "2 min"],
                [5, "5 min"],
                [15, "15 min"],
              ]}
            />
          </Row>
        )}
      </div>

      <div className="flex gap-2 rounded-xl bg-faint p-3 text-[11.5px] leading-snug text-mute">
        <Info className="mt-px size-3.5 shrink-0" />
        <p>
          Esta app nunca ve ni guarda tus credenciales: el inicio de sesión lo hace Claude Code en un perfil aislado por cuenta. El
          uso proviene del estado que el propio Claude Code reporta (límites de 5 h y semanal).
        </p>
      </div>

      <Button variant="soft" className="w-full text-bad" onClick={onQuit}>
        <Power className="size-3.5" /> Salir de Claude Usage
      </Button>
    </motion.div>
  );
}
