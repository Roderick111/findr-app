# Code Review Findings — findr-desktop

9 parallel reviewers, 2026-05-27. Tool-augmented reviewers (Security, Performance, Dead Code) ran CLI tools first, then manual review.

## Tool Results Summary

| Tool | Result |
|---|---|
| `cargo audit` | 0 vulnerabilities, 17 warnings (all transitive tauri/wry/tao deps) |
| `cargo clippy -D warnings` | 1 warning: `-> ()` in macro-generated NSPanel code |
| `cargo machete` | 1 unused dep: `anyhow` |
| `cargo bloat --release` | 19.2MB binary. Top: findr_desktop_lib 3.7MB, std 1.4MB, rustls 417KB, tauri 395KB, reqwest 375KB |
| `cargo tree --duplicates` | All transitive — no actionable project-level dupes |

## Aggregate Stats

| Severity | Count |
|---|---|
| Critical | 8 |
| High | 14 |
| Medium | 22 |
| Low | 12 |
| **Total unique** | ~45 (after dedup across reviewers) |

---

## Priority 1 — Critical (fix before release)

### C1. Search query with spaces silently fails
**Reviewers:** Ergonomics
**File:** `src-tauri/capabilities/default.json:48`
Shell scope regex `\S+` rejects any query containing spaces. Multi-word searches like "project plan" fail silently. **Breaks core functionality.**
**Fix:** Change validator to `.+` or `[\\s\\S]+`.

### C2. No sidecar timeout — hung CLI blocks app forever
**Reviewers:** Security, Error Handling, Architecture, Concurrency
**File:** `src-tauri/src/findr_client.rs:204-215`
`run_findr` awaits `rx.recv()` with no deadline. Hung sidecar = frozen app. Background daemon also blocks permanently. `_child` handle is discarded so process can't be killed.
**Fix:** Wrap in `tokio::time::timeout(Duration::from_secs(30), ...)`. Keep child handle, call `kill()` on timeout.

### C3. Background daemon: no shutdown, no timeout, no backoff
**Reviewers:** Error Handling, Architecture, Concurrency, Testability
**File:** `src-tauri/src/background.rs:6-37`
Bare `std::thread` with infinite loop, no cancellation signal, no `JoinHandle` returned. Wakes every 5s but acts every 300s (59 useless wakeups per cycle). No backoff on repeated failure. Blocks indefinitely on initial sync.
**Fix:** Use `tokio::spawn` with `CancellationToken`. Sleep `SYNC_INTERVAL` directly. Add timeout + backoff.

### C4. CSP disabled — no defense against XSS
**Reviewers:** Security
**File:** `src-tauri/tauri.conf.json:52`
`"csp": null` disables all content security policy. Combined with `/**` asset scope, any XSS = full filesystem read.
**Fix:** Set restrictive CSP: `"default-src 'self'; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'"`.

### C5. License `unknown` status bypasses gate entirely
**Reviewers:** Security, Error Handling, Testability, Ergonomics
**File:** `src/components/LicenseGate.tsx:72`
`if (status === "active" || status === "unknown") return <>{children}</>`. First launch, corrupted store, or any deserialization error = full access. Deleting `settings.json` = permanent free access.
**Fix:** Default to restricted state on `unknown`. Require explicit trial start or activation.

### C6. Panics in app setup — crash with no diagnostics
**Reviewers:** Error Handling
**File:** `src-tauri/src/lib.rs:119,186-187,178-179`
`unwrap()` on tray icon, NSPanel window lookup, and panel conversion. Any misconfiguration = silent crash on launch (no window, no error, app bounces in dock and dies).
**Fix:** Use `if let` or return `Result` with logging/dialog fallback.

### C7. Zero test infrastructure
**Reviewers:** Testability
**Files:** `package.json`, `src-tauri/Cargo.toml`
No test deps (frontend or backend). No `#[cfg(test)]` modules. No vitest/jest. License system (revenue gate) completely untestable — hardcoded clock, live HTTP, real filesystem.
**Fix:** Add vitest for frontend. Extract `trait FindrRunner` and `Clock` for Rust testability. Start with license logic tests.

### C8. `moveToTrash` tracks wrong action type
**Reviewers:** Ergonomics, Architecture, API Design
**File:** `src/App.tsx:94`
`trackAction(r.path, "open")` inside `moveToTrash`. Corrupts interaction data. Shell scope regex only allows `(open|finder|copy|preview)` — `"trash"` would be rejected.
**Fix:** Add `trash` to scope regex. Pass `"trash"` as action. Also: add confirmation before trash (currently one keystroke deletes).

---

## Priority 2 — High (fix before beta)

### H1. License key stored in plaintext
**Reviewers:** Security
**File:** `src-tauri/src/license.rs:84`, `commands.rs:176`
Raw license key persisted to `settings.json`. Any local app can steal it.
**Fix:** Don't persist raw key. Store only `activation_id` + `validated_at`.

### H2. API key passed as CLI argument — visible in `ps`
**Reviewers:** Security, API Design
**File:** `src-tauri/src/findr_client.rs:185`
OpenRouter key visible in process listing to any local user.
**Fix:** Pass via stdin or environment variable.

### H3. License HTTP calls block sync — UI freezes
**Reviewers:** Error Handling, Architecture, Concurrency
**File:** `src-tauri/src/license.rs:58-89`
`ureq::post()` is synchronous. `activate_license` and `validate_license` commands are not async. Slow/unreachable Polar API = frozen UI.
**Fix:** Mark commands `async`, use non-blocking HTTP or `spawn_blocking` with timeout.

### H4. Validation failure defaults to Active
**Reviewers:** Error Handling, Testability
**File:** `src-tauri/src/license.rs:122`
`validate_license(key, aid).unwrap_or(LicenseStatus::Active)`. Network error = permanent active status. Revoked license on offline machine stays active forever.
**Fix:** Default to current status unchanged, track that validation is overdue.

### H5. Focus handler clears query on every focus
**Reviewers:** Ergonomics
**File:** `src/App.tsx:107-120`
Alt-Tab away and back = search context wiped. Spotlight/Alfred preserve context within a session.
**Fix:** Only clear on re-show after hide, not on every focus event.

### H6. Hardcoded `⌘` symbols on all platforms
**Reviewers:** Ergonomics
**File:** `src/App.tsx:311`, `src/components/ActionsPanel.tsx:56`
Windows/Linux users see Mac Command symbol everywhere. Settings correctly detects platform but overlay/actions panel don't.
**Fix:** Add platform detection, show `Ctrl` on non-Mac.

### H7. Settings polls doctor every 2s even when hidden
**Reviewers:** Performance, Concurrency, Architecture, API Design
**File:** `src/settings/Settings.tsx:45-48`
Settings window is hidden, not destroyed. `setInterval(loadReport, 2000)` spawns sidecar process every 2s forever after first open. ~43,200 unnecessary process spawns per day.
**Fix:** Only poll when visible. Use 30s+ interval. Stop on consecutive failures.

### H8. Wildcard asset/fs scope — any file readable
**Reviewers:** Security
**File:** `src-tauri/tauri.conf.json:54-57`, `capabilities/default.json:38`
`"allow": ["/**"]` with `requireLiteralLeadingDot: false` on both asset protocol and fs. Combined with CSP=null, XSS = read any file on disk.
**Fix:** Scope to scan paths or user home directory.

### H9. Unhandled promise rejections in clipboard ops
**Reviewers:** Error Handling
**File:** `src/App.tsx:79-89`
`copyPath` and `copyFilename` call `writeText()` without try/catch. Permission denied = unhandled rejection, no user feedback.
**Fix:** Wrap in try/catch like other action handlers.

### H10. `unwrap()` on license state serialization
**Reviewers:** Error Handling
**File:** `src-tauri/src/commands.rs:166,176,184`
`serde_json::to_value(&state).unwrap()` in three places. If serialization ever fails, command panics inside async runtime.
**Fix:** Use `map_err(|e| e.to_string())?`.

### H11. `@sentry/react` and `@sentry/vite-plugin` potentially interfere with tauri-plugin-sentry
**Reviewers:** Dead Code, Architecture (+ research agent)
**File:** `package.json:13,31`
`@sentry/vite-plugin` can silently break error capture when used with `tauri-plugin-sentry`. The plugin handles JS errors via IPC — vite plugin's instrumentation interferes.
**Fix:** Remove `@sentry/vite-plugin` from vite config. Upload source maps via `sentry-cli` in CI instead. Consider removing `@sentry/react` dep if unused.

### H12. Unused `anyhow` dependency
**Reviewers:** Dead Code (TOOL-caught)
**File:** `src-tauri/Cargo.toml:30`
Declared but never used. All error handling uses `Result<_, String>`.
**Fix:** Remove from Cargo.toml.

### H13. Initial sync blocks indefinitely on first launch
**Reviewers:** Architecture, Ergonomics
**File:** `src-tauri/src/background.rs:10-13`
First launch with empty index: `sync` called with no timeout, no progress beyond initial emit, no cancellation. Frontend shows nothing — no "No results found", no "Indexing..." state.
**Fix:** Add timeout, emit progress events, show first-run empty state in UI.

### H14. No React error boundaries
**Reviewers:** Error Handling
Any uncaught render error unmounts entire React tree — blank white window, no recovery. User must force-quit.
**Fix:** Add `ErrorBoundary` component wrapping `App` and `Settings`.

---

## Priority 3 — Medium (fix before v1.0)

### M1. Stale closure in `moveToTrash` — `results.length - 2`
**File:** `src/App.tsx:97`
Uses pre-removal length. Can set selected to -1 or out of bounds.

### M2. Daemon vs user-triggered sidecar race
**File:** `src-tauri/src/background.rs`, `commands.rs`
No coordination between daemon sync and user-triggered rebuild/sync. Two sidecar processes fight over database.

### M3. TOCTOU race in `remove_scan_path`
**File:** `src-tauri/src/commands.rs:70-83`
Read-modify-write: calls `doctor()` then `rebuild()`. Concurrent removals can restore paths.

### M4. Unbounded stdout/stderr in `run_findr`
**File:** `src-tauri/src/findr_client.rs:204-215`
No size limit on accumulated output. Buggy sidecar could exhaust memory.

### M5. Information disclosure in error messages
**File:** `src-tauri/src/findr_client.rs:125,134,145`
Raw stdout included in parse errors forwarded to frontend. Could expose paths or diagnostics.

### M6. Potential panic on UTF-8 string slicing
**File:** `src-tauri/src/findr_client.rs:125`
`&stdout[..stdout.len().min(200)]` panics if byte 200 falls mid-character.

### M7. Preview reads entire file before truncating
**File:** `src/components/Preview.tsx:100-104`
`readTextFile` loads full file, then truncates to 50KB. 500MB log file = 500MB in renderer.

### M8. `App.css` dead boilerplate
**File:** `src/App.css`
117 lines of Tauri template CSS, never imported. `:root` block can conflict with `index.css`.

### M9. `IndexStatus` uses `serde(flatten)` catch-all
**File:** `src-tauri/src/findr_client.rs:37`
Opaque `other: Map` breaks type safety. No matching TypeScript type. Command unused by frontend.

### M10. `actions` array recreated every render in ActionsPanel
**File:** `src/components/ActionsPanel.tsx:44-87,133`
New array ref each render → `useEffect` re-registers keydown listener constantly.

### M11. `alert()` used for errors in Settings
**File:** `src/settings/Settings.tsx:131-133`
Native `alert()` looks jarring, blocks thread, doesn't match app design.

### M12. Update install has no error feedback
**File:** `src/components/UpdateBanner.tsx:15,27-28`
Check errors swallowed. Install failure silently resets button. No progress indication.

### M13. `remove_scan_path` implements business logic in command layer
**File:** `src-tauri/src/commands.rs:69-83`
Domain logic (filter paths, validate last path) in Tauri command instead of `findr_client.rs` or CLI.

### M14. No empty state for search results
**File:** `src/App.tsx:252-277`
Empty results = blank panel. No "No results" message, no indexing hint for first-run.

### M15. Shell scope validators overly permissive
**File:** `src-tauri/capabilities/default.json`
`add-path` uses `.+`, `set-key` uses `.+`. No path traversal or format validation.

### M16. Cmd+C may intercept native copy in search input
**File:** `src/App.tsx:190-195`
`window.getSelection()` doesn't reliably detect input element selections in all browsers.

### M17. `chrono` serde feature enabled but unused
**File:** `src-tauri/Cargo.toml:29`
`features = ["serde"]` pulls extra code. Chrono only used for `to_rfc3339()` / `parse_from_rfc3339()`.

### M18. Duplicate `formatBytes`/`formatDate` utilities
**Files:** `src/components/Preview.tsx:44,52` and `src/settings/Settings.tsx:386,392`
Same logic, slightly different implementations.

### M19. Tauri IPC parameter naming — no type safety
**File:** `src/App.tsx:131-134`
`invoke()` args have no TypeScript type checking. Typo in param name = silent `undefined`.

### M20. Blocking HTTP in `check_license_state` via `validate_license`
**File:** `src-tauri/src/license.rs:114-132`
Called from sync command `get_license_state`, which Settings polls every 2s. If offline grace expires during polling, UI hangs.

### M21. `crate-type` includes `staticlib` and `cdylib`
**File:** `src-tauri/Cargo.toml:10`
For iOS/Android builds. Desktop-only project compiles unnecessary artifacts.

### M22. Unused CSS custom properties
**File:** `src/index.css:16,18,26`
`--border-focus`, `--accent-hover`, `--input-bg` defined but never referenced.

---

## Priority 4 — Low (nice to have)

### L1. tauri-nspanel pinned to git branch, not commit hash
**File:** `src-tauri/Cargo.toml:36`
Supply chain risk — branch can be force-pushed.

### L2. NSPanel handler lifetime — potential dangle
**File:** `src-tauri/src/lib.rs:200-207`
`FindrPanelEventHandler` created as local, ref passed to panel. Lifetime depends on plugin internals.

### L3. `toggle_overlay` indentation mismatch
**File:** `src-tauri/src/lib.rs:225-226`
`make_key_window()` correctly inside `else` but indentation misleading.

### L4. `let _ =` on window operations — silent failures
**File:** `src-tauri/src/lib.rs:81,85,149,233-235`
Window show/hide/focus errors completely invisible. No logging.

### L5. `navigator.platform` deprecated
**File:** `src/settings/Settings.tsx:227-228`
Should use `navigator.userAgentData?.platform` with fallback.

### L6. glib 0.18.5 unsoundness advisory (RUSTSEC-2024-0429)
Transitive via tauri. Not exploitable in this app.

### L7. Duplicate icon/extension mapping
**Files:** `src/App.tsx:17-27`, `src/components/Preview.tsx:17-27`
Slightly different extension lists. Should extract to shared utility.

### L8. `synchronous getCurrentWindow()` at module top level
**File:** `src/main.tsx:10`
Runs before React mounts. Failure = no error boundary possible.

### L9. Trial off-by-one at expiry boundary
**File:** `src-tauri/src/license.rs:142-150`
`num_days()` truncates. 13 days 23 hours = 1 day remaining. No test pins expected behavior.

### L10. Unused Tauri plugin JS packages
**File:** `package.json:15,19`
`@tauri-apps/plugin-autostart` and `plugin-global-shortcut` JS packages never imported in frontend.

### L11. `reqwest` compiled but only used transitively
375KB in binary via `tauri-plugin-updater`. `ureq` handles direct HTTP. Not actionable but tracked.

### L12. `POLAR_ORG_ID` hardcoded — no staging environment support
**File:** `src-tauri/src/license.rs:5`
Minor. Prevents testing against staging API.

---

## Positive Observations

- No `unsafe` blocks in first-party Rust code
- No `dangerouslySetInnerHTML` in React
- Sidecar uses Tauri shell scope (execve, not shell) — no command injection
- `trash` crate for safe OS trash API
- Updater uses pubkey verification
- `from_utf8_lossy` prevents panics on invalid UTF-8
- Clean sidecar IPC abstraction in `findr_client.rs`
- CSS design system well-structured with custom properties
- Theme sync via events (no polling/flash)
