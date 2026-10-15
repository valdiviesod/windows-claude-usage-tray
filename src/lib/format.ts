export function fmtRemaining(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return `${d} d ${h} h`;
  if (h > 0) return `${h} h ${m} min ${sec} s`;
  if (m > 0) return `${m} min ${sec} s`;
  return `${sec} s`;
}

export function fmtClock(epochSecs: number, nowMs: number): string {
  const date = new Date(epochSecs * 1000);
  const time = date.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit", hour12: false });
  const sameDay = date.toDateString() === new Date(nowMs).toDateString();
  if (sameDay) return time;
  const day = date.toLocaleDateString("es", { weekday: "short" });
  return `${day} ${time}`;
}

export function fmtAgo(epochSecs: number, nowMs: number): string {
  const diff = Math.max(0, Math.floor(nowMs / 1000 - epochSecs));
  if (diff < 5) return "ahora";
  if (diff < 60) return `hace ${diff} s`;
  if (diff < 3600) return `hace ${Math.max(1, Math.round(diff / 60))} min`;
  if (diff < 86400) return `hace ${Math.round(diff / 3600)} h`;
  return `hace ${Math.round(diff / 86400)} d`;
}

export function fmtPlan(plan: string | null): string | null {
  if (!plan) return null;
  const known: Record<string, string> = { pro: "Pro", max: "Max", team: "Team", enterprise: "Enterprise", free: "Free" };
  return known[plan.toLowerCase()] ?? plan.charAt(0).toUpperCase() + plan.slice(1);
}

export const pct = (n: number) => `${Math.round(n)}%`;
