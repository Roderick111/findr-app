# findr Desktop

Tauri 2 + React 19 desktop app wrapping the findr CLI as a sidecar. All search/index logic is in the CLI — this repo is purely UI.

## Build & Run

```bash
bun install
bun run tauri dev      # dev mode

# Release build — ALWAYS use ad-hoc signing
APPLE_SIGNING_IDENTITY="-" bun run tauri build

# Post-build: hide .VolumeIcon.icns in DMG
cd src-tauri && bash fix-dmg.sh
```

**Code signing is mandatory.** Without `APPLE_SIGNING_IDENTITY="-"`, macOS shows "app is damaged" and refuses to open. With it, users get "unidentified developer" warning — right-click > Open works. Full notarization requires $99/yr Apple Developer cert (not set up yet).

**Before building:** eject any mounted findr DMG volumes (`hdiutil detach /Volumes/findr`) or `bundle_dmg.sh` will fail.

Dev sidecar binary is symlinked: `src-tauri/binaries/findr-aarch64-apple-darwin -> findr-app/target/release/findr`

## Website

```bash
cd website && bash deploy.sh
```

Single-page site at https://findr.beautiful-apps.com/. Deploy script copies DMG from build output, rsyncs to server, rebuilds Docker container.

## Architecture Rules

- **Never duplicate core logic.** Search, indexing, OCR, embeddings — all via sidecar. Desktop only does UI, licensing, and system integration.
- **Sidecar IPC** goes through `findr_client.rs` → `run_findr()`. All commands spawn the sidecar, collect stdout/stderr, parse JSON. Adding a new CLI command means: (1) add function in `findr_client.rs`, (2) add `#[tauri::command]` in `commands.rs`, (3) register in `lib.rs` invoke_handler, (4) add shell scope entry in `capabilities/default.json`.
- **Two windows** share one React app. `main.tsx` checks `getCurrentWindow().label` to render `App` (search) or `Settings`.

## Scope Gotcha

Tauri 2 on Unix defaults `require_literal_leading_dot: true`. **Every plugin** (fs, opener, asset protocol) needs `requireLiteralLeadingDot: false` in its own config section of `tauri.conf.json`. The `/**` glob alone is not enough for dotfile paths. See `DESKTOP_APP_PLAN.md` for the exact config.

## Shell Scope

`capabilities/default.json` has regex-validated entries for each sidecar argument pattern. Adding a new findr subcommand or flag requires a matching scope entry or the call will be rejected at runtime.

## Settings Window

Uses `prevent_close` + `hide()` at the builder level (`on_window_event` in `lib.rs`). Do not use per-window `on_window_event` — it gets dropped.

## Theme

Stored in plugin-store. Cross-window sync via `app.emit("theme-changed", &theme)`. Do not poll on focus — causes visible flash.

## Sentry

Crash reporting via `tauri-plugin-sentry`. Rust side: `sentry::init()` in `main.rs` captures panics + native minidumps. JS side: plugin injects its own SDK that forwards webview errors to Rust via IPC — **do not** add a separate `@sentry/react` `Sentry.init()` or `@sentry/vite-plugin`, they conflict with the plugin's IPC transport. Source maps uploaded via `sentry-cli` in CI, not vite plugin.

## Testing Changes

After modifying Rust code, `cargo build` from `src-tauri/`. After modifying React code, HMR picks it up automatically. Config changes (`tauri.conf.json`, `capabilities/`) require full restart.
