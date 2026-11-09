import { motion } from "motion/react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

export function IconButton({
  label,
  children,
  className = "",
  ...rest
}: { label: string; children: ReactNode } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      aria-label={label}
      title={label}
      className={`grid size-6 place-items-center border border-transparent text-mute transition-colors hover:border-line hover:bg-faint hover:text-accent active:scale-95 disabled:opacity-40 ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export function Button({
  variant = "soft",
  className = "",
  ...rest
}: { variant?: "primary" | "soft" | "ghost" } & ButtonHTMLAttributes<HTMLButtonElement>) {
  const styles = {
    primary: "border-accent/60 bg-accent/10 text-accent hover:border-accent hover:bg-accent hover:text-on-accent",
    soft: "border-line bg-faint text-ink hover:border-line-hi hover:bg-card-hi",
    ghost: "border-transparent text-mute hover:border-line hover:bg-faint hover:text-ink",
  }[variant];
  return (
    <button
      className={`inline-flex h-7 items-center justify-center gap-1.5 border px-2.5 text-[10.5px] font-bold uppercase tracking-[0.14em] transition active:scale-[0.98] disabled:opacity-45 ${styles} ${className}`}
      {...rest}
    />
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-[18px] w-[36px] shrink-0 border transition-colors duration-150 ${
        checked ? "border-accent bg-accent/15" : "border-line-hi bg-faint"
      }`}
    >
      <motion.span
        className={`absolute left-[2px] top-[2px] size-[12px] ${checked ? "bg-accent" : "bg-mute"}`}
        style={{ boxShadow: checked ? "0 0 10px rgba(125,174,163,0.65)" : "none" }}
        animate={{ x: checked ? 16 : 0 }}
        transition={{ type: "spring", stiffness: 520, damping: 36 }}
      />
    </button>
  );
}

export function Bar({ value, tone, thin = false }: { value: number; tone: "ok" | "warn" | "bad"; thin?: boolean }) {
  const color = { ok: "var(--ok)", warn: "var(--warn)", bad: "var(--bad)" }[tone];
  const filled = Math.min(100, Math.max(value > 0 ? 1.5 : 0, value));

  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(value)}
      aria-valuemin={0}
      aria-valuemax={100}
      className={`meter w-full overflow-hidden bg-[var(--faint)] ${thin ? "h-[6px]" : "h-[9px]"}`}
    >
      <motion.div
        className="h-full"
        style={{ background: color, boxShadow: `0 0 14px ${tone === "ok" ? "rgba(169,182,101,0.5)" : tone === "warn" ? "rgba(216,166,87,0.55)" : "rgba(234,105,98,0.6)"}` }}
        initial={false}
        animate={{ width: `${filled}%` }}
        transition={{ type: "spring", stiffness: 160, damping: 26 }}
      />
    </div>
  );
}

/** Small monospace label with a leading tick, used above figures and rows. */
export function Eyebrow({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <div className={`flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.2em] text-mute ${className}`}>
      <span className="text-accent/70">::</span>
      {children}
    </div>
  );
}

/** Nerd Font glyph. */
export function Glyph({ code, className = "" }: { code: string; className?: string }) {
  return <span className={`font-mono leading-none ${className}`}>{code}</span>;
}
