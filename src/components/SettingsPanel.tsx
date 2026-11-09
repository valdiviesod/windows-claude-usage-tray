import { motion } from "motion/react";
import type { ReactNode } from "react";
import type { Settings } from "../lib/types";
import { Button, Glyph, IconButton, Toggle } from "./ui";

interface Props {
  settings: Settings;
  onChange: (s: Settings) => void;
  onBack: () => void;
  onQuit: () => void;
}

const BACK = "\uf053";
const INFO = "\uf05a";
const POWER = "\uf011";
const CARET = "\uf0d7";

function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="min-w-0">
        <div className="text-[10.5px] font-bold uppercase tracking-[0.12em]">{title}</div>
        {hint && <div className="mt-1 text-[9.5px] leading-snug text-mute">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

function Select<T extends number>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="relative shrink-0">
      <select
        aria-label={label}
        value={value}
        onChange={(e) => onChange(Number(e.target.value) as T)}
        className="num h-[24px] appearance-none border border-line bg-panel pl-2 pr-6 text-[10px] font-bold uppercase tracking-[0.1em] text-ink outline-none transition-colors hover:border-line-hi focus:border-accent"
      >
        {options.map(([v, text]) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
      <Glyph
        code={CARET}
        className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 text-[9px] text-mute"
      />
    </div>
  );
}

export function SettingsPanel({ settings, onChange, onBack, onQuit }: Props) {
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => onChange({ ...settings, [k]: v });
  return (
    <motion.div initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.16 }} className="space-y-2.5">
      <div className="flex items-center gap-1">
        <IconButton label="Volver" onClick={onBack}>
          <Glyph code={BACK} className="text-[11px]" />
        </IconButton>
        <h2 className="text-[10.5px] font-extrabold uppercase tracking-[0.2em]">configuración</h2>
        <span className="num ml-auto text-[9px] uppercase tracking-[0.14em] text-mute">~/tray</span>
      </div>

      <div className="divide-y divide-line border border-line bg-card px-2.5">
        <Row title="actualización" hint="lectura local de los datos de cada cuenta.">
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
        <Row title="iniciar con windows" hint="arranca oculto en la bandeja.">
          <Toggle label="Iniciar con Windows" checked={settings.autostart} onChange={(v) => set("autostart", v)} />
        </Row>
        <Row title="notificaciones" hint="al llegar a 80%, 95% y 100%.">
          <Toggle label="Notificaciones" checked={settings.notifications} onChange={(v) => set("notifications", v)} />
        </Row>
        <Row
          title="consulta directa"
          hint="solo cuando claude code no reporta datos recientes. petición mínima (<0,1% por consulta)."
        >
          <Toggle label="Consulta directa automática" checked={settings.probe_enabled} onChange={(v) => set("probe_enabled", v)} />
        </Row>
        {settings.probe_enabled && (
          <Row title="cada">
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

      <div className="flex gap-2 border-l-2 border-accent bg-faint px-2.5 py-2 text-[9.5px] leading-snug text-mute">
        <Glyph code={INFO} className="mt-px text-[10px] text-accent" />
        <p>
          esta app nunca ve ni guarda tus credenciales: el login lo hace claude code en un perfil aislado por cuenta. el uso
          proviene de lo que el propio claude code reporta (límites de 5 h y semanal).
        </p>
      </div>

      <Button variant="soft" className="w-full border-bad/30 text-bad hover:border-bad/60 hover:bg-bad/10" onClick={onQuit}>
        <Glyph code={POWER} className="text-[10px]" /> salir de claude usage
      </Button>
    </motion.div>
  );
}
