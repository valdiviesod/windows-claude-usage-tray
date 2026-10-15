import { useEffect, useRef, useState } from "react";
import { api, on } from "./api";
import type { AppView } from "./types";

/** Backend-pushed state. No polling: Rust emits `state-changed`. */
export function useAppState() {
  const [state, setState] = useState<AppView | null>(null);

  useEffect(() => {
    let alive = true;
    api.getState().then((s) => alive && setState(s));
    const off = on<AppView>("state-changed", (s) => alive && setState(s));
    return () => {
      alive = false;
      off.then((f) => f());
    };
  }, []);

  return [state, setState] as const;
}

/**
 * Wall clock for countdowns. Ticks only while the window is visible, so a
 * hidden window costs nothing.
 */
export function useNow(periodMs = 1_000) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    let timer: number | undefined;
    const start = () => {
      stop();
      setNow(Date.now());
      timer = window.setInterval(() => setNow(Date.now()), periodMs);
    };
    const stop = () => {
      if (timer !== undefined) window.clearInterval(timer);
      timer = undefined;
    };
    const sync = () => (document.visibilityState === "visible" ? start() : stop());
    const offShown = on("window-shown", start);
    const offHidden = on("window-hidden", stop);
    document.addEventListener("visibilitychange", sync);
    sync();
    return () => {
      stop();
      document.removeEventListener("visibilitychange", sync);
      offShown.then((f) => f());
      offHidden.then((f) => f());
    };
  }, [periodMs]);

  return now;
}

/** Keep the native window as tall as the content (compact, adaptive). */
export function useFitWindow<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let last = 0;
    let raf = 0;
    const fit = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const h = Math.ceil(el.getBoundingClientRect().height) + 24; // shadow gutter
        if (Math.abs(h - last) >= 1) {
          last = h;
          api.fitWindow(h);
        }
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

  return ref;
}
