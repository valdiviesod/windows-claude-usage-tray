// Entry point of the `pill` webview: the taskbar usage readout.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { UsagePill } from "./components/UsagePill";
import "./index.css";

async function boot() {
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    (await import("./dev-mock")).installDevMock();
  }
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <UsagePill />
    </StrictMode>,
  );
}

void boot();
