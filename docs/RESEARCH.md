# Research: how to read real Claude Pro/Max usage

Date of research: 2026-10-09. Goal: show, for two independent Claude subscription
accounts, the 5-hour window, weekly limit and reset times, with verifiable data.

## Findings

| Candidate source | Official / documented? | Allowed for third-party apps? | Decision |
|---|---|---|---|
| **Claude Code statusline JSON** (`rate_limits.five_hour` / `seven_day`: `used_percentage` 0-100, `resets_at` epoch s) | Yes, documented at <https://code.claude.com/docs/en/statusline> | Yes: runs inside the unmodified Claude Code binary the user signed in to | **Primary source** |
| `claude auth status` / `claude auth login` / `claude auth logout` | Yes (official CLI) | Yes: sign-in completes through Anthropic's own flow | Used for connect / disconnect / identity |
| `claude -p ... --output-format stream-json` emits `rate_limit_event` with `unifiedWindows.{five_hour,seven_day}.utilization` (0-1 fraction) and `resetsAt` | Event type is in the Agent SDK reference; the `unifiedWindows` field is **not documented** | Same binary, but each call is a real request that consumes quota | Opt-in / user-triggered only ("Consultar ahora") |
| `GET https://api.anthropic.com/api/oauth/usage` with a Claude OAuth token + `anthropic-beta: oauth-2025-04-20` | **No** (reverse-engineered; schema changes, aggressive 429s) | **No** (see below) | **Not used** |
| Own OAuth flow / Windows Credential Manager for Claude tokens | n/a | **No** (see below) | **Not implemented** |

### Why there is no custom OAuth login

Anthropic's Claude Code legal page
(<https://code.claude.com/docs/en/legal-and-compliance>, "Authentication and credential use")
states that OAuth authentication is intended for ordinary use of Claude Code and native
Anthropic apps, that Anthropic *"does not permit third-party developers to offer Claude.ai
login into their own applications, or to route requests through Free, Pro, or Max plan
credentials on behalf of their users"*, and that developers *"may not collect, store, or
intermediate Claude.ai credentials or session tokens — sign-in to a Claude account must
complete through Anthropic's own flow."*

Therefore this app never touches Claude credentials. Each account is a separate
`CLAUDE_CONFIG_DIR` (Claude Code's own credential store, isolated per account). The
requirement "store credentials in Windows Credential Manager" is intentionally replaced by
"store nothing": the app holds no secrets.

### Unit verification of the probe

The statusline reported `used_percentage: 43` while a `claude -p` probe made moments
later reported `utilization: 0.43` for the same window (same `resetsAt`). So the probe's
`utilization` is a 0-1 fraction and is multiplied by 100. Verified empirically on
2026-10-09 against Claude Code 2.1.295 using a pseudo-terminal session.

## Real limitations (shown honestly in the UI)

1. **Statusline data exists only after a Claude Code session of that account made an API
   call.** Claude Code does not report limits otherwise, and drops a window once its
   `resets_at` passes. Between sessions the tray shows the last observation with its age;
   after a reset it shows "No disponible" rather than inventing a value.
2. Limits are only exposed for claude.ai Pro/Max accounts (not API-key logins). The app
   rejects accounts whose `authMethod` is not `claude.ai`.
3. The probe costs a (tiny) real request against the subscription. Default: off. The
   manual "Consultar ahora" button works without enabling it.
4. Weekly/per-model limits beyond `seven_day` are not exposed by the documented fields.
5. Everything depends on Claude Code's output format. If it changes, the parsers fail
   closed (no data) instead of showing wrong numbers.
