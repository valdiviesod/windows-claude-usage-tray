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
      className={`grid size-6 place-items-center rounded-md text-mute transition-colors hover:bg-faint hover:text-ink active:scale-95 disabled:opacity-40 ${className}`}
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
    primary: "bg-accent text-on-accent hover:brightness-110",
    soft: "bg-faint text-ink hover:bg-line",
    ghost: "text-mute hover:text-ink hover:bg-faint",
  }[variant];
  return (
    <button
      className={`inline-flex h-7 items-center justify-center gap-1.5 rounded-md px-3 text-[12.5px] font-medium transition active:scale-[0.97] disabled:opacity-50 ${styles} ${className}`}
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
      className={`relative h-[22px] w-[36px] shrink-0 rounded-full transition-colors duration-200 ${checked ? "bg-switch" : "bg-faint"}`}
    >
      <motion.span
        className="absolute left-[2px] top-[2px] size-[18px] rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.3)]"
        animate={{ x: checked ? 14 : 0 }}
        transition={{ type: "spring", stiffness: 500, damping: 34 }}
      />
    </button>
  );
}

export function Bar({ value, tone, thin = false }: { value: number; tone: "ok" | "warn" | "bad"; thin?: boolean }) {
  const color = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad" }[tone];
  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(value)}
      aria-valuemin={0}
      aria-valuemax={100}
      className={`w-full overflow-hidden rounded-full bg-faint ${thin ? "h-1" : "h-1.5"}`}
    >
      <motion.div
        className={`h-full rounded-full ${color}`}
        initial={false}
        animate={{ width: `${Math.min(100, Math.max(value > 0 ? 1.5 : 0, value))}%` }}
        transition={{ type: "spring", stiffness: 140, damping: 24 }}
      />
    </div>
  );
}
