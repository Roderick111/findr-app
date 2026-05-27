# findr Desktop

Native macOS desktop app for [findr](https://github.com/Roderick111/findr-app) — fast local file search with preview. Built with Tauri 2 + React 19.

## Features

- **Spotlight-style overlay** — `Cmd+Shift+F` to toggle, works over fullscreen apps (macOS NSPanel)
- **First-run onboarding** — one click to index home folder, search works in seconds
- **File preview** — images, markdown, code, text, PDFs rendered inline (50KB cap for text)
- **Actions panel** — `Cmd+K` for quick actions: open, reveal in Finder, copy path, move to trash
- **Background indexing** — automatic sync every 5 minutes with exponential backoff
- **Semantic search** — optional OpenRouter API key for vector-based search
- **Light/dark/system themes** — synced across search and settings windows
- **Auto-updater** — checks GitHub releases, downloads and installs in-app
- **Crash reporting** — Sentry via `tauri-plugin-sentry`
- **Error boundaries** — React errors caught with recovery UI

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Cmd+Shift+F` | Toggle overlay |
| `Enter` | Open selected file |
| `Cmd+Enter` | Reveal in Finder |
| `Cmd+K` / `Tab` | Open actions panel |
| `Cmd+C` | Copy path |
| `Cmd+Shift+C` | Copy filename |
| `Cmd+Backspace` | Move to trash |
| `Cmd+,` | Open settings |
| `Esc` | Hide overlay |

## Development

Requires [Rust](https://rustup.rs/), [Bun](https://bun.sh/), and the findr CLI binary.

```bash
bun install

# Place findr binary (or symlink for dev)
ln -sf /path/to/findr/target/release/findr src-tauri/binaries/findr-aarch64-apple-darwin

bun run tauri dev
```

## Testing

```bash
# Frontend (65 tests)
bun run test

# Rust (70 tests)
cd src-tauri && cargo test
```

## Build

```bash
bun run tauri build
```

CI builds via GitHub Actions on tag push (`v*`). Produces macOS arm64 + x86_64 binaries.

## Architecture

Thin GUI shell. All search and indexing logic lives in the [findr CLI](https://github.com/Roderick111/findr-app), bundled as a Tauri sidecar binary.

```
src/              React frontend (search overlay + settings window)
src-tauri/src/    Rust backend (sidecar IPC, licensing, NSPanel, tray)
```
