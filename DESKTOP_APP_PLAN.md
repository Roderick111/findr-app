# findr Desktop — PRD

## Overview

findr Desktop is a Tauri 2 + React 19 app that wraps the findr CLI as a native overlay for macOS. It provides instant file search with rich preview, keyboard-driven navigation, and background indexing.

## Architecture

### Two-Process Model

The desktop app never touches the search index directly. All data flows through the findr CLI binary bundled as a Tauri sidecar:

```
[React Frontend] → invoke() → [Rust Commands] → sidecar spawn → [findr CLI] → stdout JSON → [Rust] → [React]
```

This means:
- Desktop and CLI can be versioned independently
- CLI improvements (search quality, indexing speed) are automatically inherited
- Desktop is purely UI — no search logic, no SQLite access, no index management

### Window Architecture

Two Tauri windows defined in `tauri.conf.json`:

1. **main** — Search overlay. Transparent, always-on-top, no decorations. On macOS uses NSPanel via `tauri-nspanel` for fullscreen-app support. Hidden on blur/escape.
2. **settings** — Standard decorated window. Uses `prevent_close` + `hide()` pattern so it survives close button clicks and can be reopened.

Both share the same React app via `main.tsx` — window label determines which component renders.

### Theme System

- Preference stored in Tauri plugin-store (`settings.json`)
- `useThemeProvider()` hook reads on mount, applies `data-theme` attribute
- Cross-window sync via Tauri event: `set_theme` command emits `theme-changed`, all windows listen
- CSS custom properties in `index.css` handle light/dark/system

### Background Daemon

`background.rs` spawns a tokio task with `AtomicBool` shutdown flag:
1. On startup: checks index status, runs initial sync if empty (with 30s timeout)
2. Every 5 minutes: runs `findr index sync` to pick up filesystem changes
3. Emits `index-sync` events for the settings UI status display
4. Exponential backoff on consecutive failures (capped at 10 minutes)
5. `SyncLock` prevents races between daemon and user-triggered sync/rebuild

### Licensing

- **Polar.sh API** for activation/validation
- **Machine fingerprint** via SHA-256 of platform UUID (ioreg on macOS, registry on Windows, /etc/machine-id on Linux)
- **States:** unknown → trial (14 days) → trial_expired / active
- **Offline grace:** 7 days before re-validation required
- **Storage:** `settings.json` via Tauri plugin-store

## Frontend Components

| Component | Role |
|---|---|
| `App.tsx` | Search input, result list, preview pane, keyboard handler, toast system, first-run onboarding |
| `Preview.tsx` | File preview — images via asset protocol, text/code/markdown via fs `open()`+`read()` (50KB cap), PDF via embed |
| `ActionsPanel.tsx` | Cmd+K popup — open, reveal, copy path/filename, trash, settings |
| `ErrorBoundary.tsx` | Catches React render errors, shows fallback UI with reload button |
| `LicenseGate.tsx` | License gate (currently disabled for testing) |
| `UpdateBanner.tsx` | Auto-update banner at top of overlay |
| `Settings.tsx` | Scan paths, theme, hotkey, autostart, semantic search, index status, reindex, license, about |

### Hooks

| Hook | Role |
|---|---|
| `useTheme` | Theme context consumer + provider with cross-window event sync |
| `useDebounced` | Generic debounce for search input (200ms) |

## Rust Commands

All in `commands.rs`, registered in `lib.rs`:

| Command | Sidecar Call | Notes |
|---|---|---|
| `search` | `findr search <q> --json --limit N [--no-semantic]` | |
| `get_recent_files` | `findr search "" --json --limit N` | Empty query = recent mode |
| `track_interaction` | `findr track <path> --action <type>` | Boosts file in rankings |
| `get_index_status` | `findr index status --json` | |
| `get_findr_version` | `findr --version` | |
| `get_doctor_report` | `findr doctor --json` | Full system health report |
| `add_scan_path` | `findr index add-path <path>` | |
| `remove_scan_path` | `findr index rebuild --paths <remaining>` | Rebuilds with path excluded |
| `run_reindex` | `findr index rebuild` | |
| `run_sync` | `findr index sync` | |
| `set_api_key` | `findr config set-key <key>` | OpenRouter for semantic search |
| `get_api_key_status` | `findr config get-key` | |
| `move_to_trash` | — | Uses `trash` crate directly, not sidecar |
| `hide_overlay` | — | NSPanel hide on macOS, window hide elsewhere |
| `open_settings` | — | Show + focus settings window |
| `get/set_theme` | — | Plugin-store + event emit |
| `get/set_autostart` | — | Tauri autostart plugin |
| `get_license_state` | — | Reads + validates license from store |
| `activate_license` | — | Polar.sh API call |
| `start_trial` | — | Creates trial state in store |
| `get_home_dir` | — | Returns user home directory via `dirs` crate |

## Tauri Scope Configuration

### Critical Gotcha: Dotfile Paths

Tauri 2 on Unix defaults `require_literal_leading_dot: true`. Glob `/**` won't match `~/.claude/`, `~/.config/`, etc. **Every plugin** that touches user files needs this disabled independently:

```json
// tauri.conf.json
{
  "plugins": {
    "fs": { "requireLiteralLeadingDot": false },
    "opener": { "requireLiteralLeadingDot": false }
  },
  "app": {
    "security": {
      "assetProtocol": {
        "scope": {
          "allow": ["/**"],
          "requireLiteralLeadingDot": false
        }
      }
    }
  }
}
```

### Sidecar Permissions

Shell execute permissions in `capabilities/default.json` use regex validators for each allowed argument pattern. Adding a new sidecar command requires a matching scope entry.

## CSS Design System

All colors via CSS custom properties in `index.css`. Two theme blocks: `:root, [data-theme="light"]` and `[data-theme="dark"]`. Categories:
- `--bg-*` — backgrounds (primary, secondary, tertiary, hover, active)
- `--text-*` — text (primary, secondary, tertiary)
- `--icon-*` — file type icon colors
- `--md-*` — markdown preview styling
- `--kbd-*` — keyboard shortcut badges
- `--overlay-*` / `--toast-*` / `--preview-*` — component-specific

### Backdrop Blur Keepalive

WebKit drops `backdrop-filter` on idle compositing layers. Fixed with a CSS animation:
```css
animation: backdrop-keepalive 1s steps(2) infinite;
```

## CI/CD

GitHub Actions workflow (`.github/workflows/release.yml`):
1. Triggered by `v*` tag push
2. Matrix: macOS arm64, macOS x86_64 (Mac-only for now)
3. Reads pinned findr version from `findr_version.txt`
4. Downloads `findr-macos-arm64`, `findr-macos-x86_64`, and `findr-ocr-macos-universal` from core repo releases
5. OCR universal binary copied to both arch targets
6. Runs `tauri-action` — builds, signs, creates draft GitHub release with updater JSON

## Remaining Work

### Desktop
- [x] Sentry crash reporting (tauri-plugin-sentry, errors forwarded via IPC to Rust client, shipped 2026-05-27)
- [x] Code review fixes — 39 findings fixed (security, sidecar robustness, license, UX), shipped 2026-05-27
- [x] Test suite — 135 tests (70 Rust + 65 vitest), shipped 2026-05-27
- [x] First-run onboarding — auto-index home folder, shipped 2026-05-27
- [x] App icon — cyan magnifying glass with search lines, shipped 2026-05-27
- [x] Website — findr.beautiful-apps.com, single-page landing, AJTBD copy, free DMG download, shipped 2026-05-27
- [x] Ad-hoc code signing — `APPLE_SIGNING_IDENTITY="-"`, "unidentified developer" instead of "damaged", shipped 2026-05-27
- [x] DMG cleanup — `chflags hidden` on .VolumeIcon.icns via post-build fix-dmg.sh, shipped 2026-05-27
- [x] Release profile — thin LTO, 16 codegen units, strip symbols (~40% faster builds), shipped 2026-05-27
- [x] fs:allow-open — added to capabilities for file preview ACL, shipped 2026-05-27
- [ ] Add `--wait 3` to mutating sidecar calls (add_path, rebuild, sync) for lock contention edge cases
- [ ] Customizable hotkey (UI placeholder exists)
- [ ] Better error UX when sidecar binary missing or crashes
- [ ] Re-enable license gate with proper unknown→activation flow
- [ ] Apple Developer cert ($99/yr) for code signing + notarization — no free alternative

### Depends on Core
- [x] Lock scope narrowing (shipped 2026-05-27)
- [x] Better lock error messages (shipped 2026-05-27)
- [x] JSONL excluded from recent (shipped 2026-05-27)
- [ ] Deleted files excluded from recent results
- [ ] `--wait <seconds>` flag for lock retry

### Nice to Have
- [ ] File type filters in search
- [ ] Drag-and-drop from result list
- [ ] Clipboard history integration
- [ ] Quick Look style preview (spacebar)
