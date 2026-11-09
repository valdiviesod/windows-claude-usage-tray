# AGENTS.md — windows-claude-usage-tray

Tauri + Vite (TypeScript) tray app. Sources: `src/` (UI), `src-tauri/` (Rust). Docs: `docs/`, `README.md`.

<!-- token-efficiency -->
## Token Efficiency
Global rules: `~/.ai-token-optimization/TOKEN_RULES.md`.
- Search narrowly: `ai search "sym" src/ src-tauri/src/`; skip `node_modules`, `dist`, `src-tauri/target`, `package-lock.json`.
- Verify per change: `npm run typecheck`. Unit boundary: `npm run build`. No test suite exists.
- Long output: `ai log -- <cmd>`.