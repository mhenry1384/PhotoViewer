# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Windows desktop photo viewer built with Tauri v2 (Rust backend) + Vite (vanilla JS frontend, no framework). It's meant to replace the default Windows Photo Viewer: it's registered as the file-association handler for JPG/PNG/HEIC, opens directly to the double-clicked image, and lets you arrow through the rest of that folder.

## Commands

- `npm run dev` — Vite dev server only (frontend hot-reload in a browser tab; no Tauri window, no Rust commands available).
- `npm run tauri dev` — full dev run: starts Vite and launches the actual Tauri window with hot reload. To simulate opening a file via file association, pass a path after a double `--` (one for npm, one for the Tauri CLI): `npm run tauri dev -- -- "C:\path\to\file.jpg"`.
- `npm run build` — Vite production build only, writes `dist/`.
- `npm run tauri build` — full release build: compiles Rust in release mode, then produces NSIS (`.exe`) and MSI installers under `src-tauri/target/release/bundle/`.
- `cargo check` / `cargo build` (run from `src-tauri/`) — fast Rust-only compile check without going through the Tauri CLI or bundling.

There is no test suite, linter, or formatter configured in this repo.

### Important: file associations only update via the installer

`bundle.fileAssociations` in `src-tauri/tauri.conf.json` is only written to the Windows registry when the generated installer is *run* — `cargo build` or `npm run tauri dev` never touch it. If you add/change an extension association, you must `npm run tauri build` and then actually run the resulting `PhotoViewer_*-setup.exe` (or `.msi`) for Explorer's right-click "Open with" list to reflect it.

## Architecture

**Frontend** (`src/main.js`, vanilla JS, single file, no framework): renders one `<img>` and a filename/counter info bar (markup in `index.html`, styles in `src/styles.css`). All backend interaction goes through `invoke()`/`convertFileSrc()` from `@tauri-apps/api`.

**Backend** (`src-tauri/src/`): `main.rs` is a one-line entry point that calls `photo_viewer_lib::run()`; almost all logic lives in `lib.rs` (the crate is named `photo_viewer_lib` and built as `staticlib`/`cdylib`/`rlib`).

### Startup / navigation flow

- Windows passes the double-clicked file path as `argv[1]`. `run()` reads this once at startup into `AppState.initial_file`.
- Frontend calls `get_initial_file` once, then `get_images_in_folder(folder)` to get every JPG/PNG/HEIC/HEIF sibling in that same directory, sorted case-insensitively — this list (`images` in `main.js`) is the whole navigation model.
- Arrow keys call `navigate(delta)`, which wraps the index modulo `images.length` in both directions.
- Delete calls `trash_file` (via the `trash` crate — recoverable, not a permanent delete) and reindexes; Escape calls `cleanup_temp_files` then closes the window (`core:window:allow-close` permission, granted in `src-tauri/capabilities/default.json`).

### HEIC/HEIF support

WebView2 can't render HEIC natively, so it's decoded on the Rust side using the Windows Imaging Component (WIC) via the `windows` crate — this depends on the OS having the "HEIF Image Extensions" codec installed (present by default on most Windows 10/11 installs, but not guaranteed). The `image` crate is only used to re-encode the decoded pixels as JPEG, not to decode HEIC itself.

- `get_display_path(file_path)` is the single chokepoint the frontend calls before displaying anything: for HEIC/HEIF it converts (or reuses a cached conversion) and returns a JPEG path; for everything else it returns the path unchanged.
- Conversions are cached in a per-process temp directory (`%TEMP%\photo_viewer_<pid>`), keyed by a hash of the original file path (`temp_path_for`) — same source file never gets converted twice in one run.
- Photos are downscaled to `HEIC_MAX_DIM` (2048px on the long edge) during decode to keep conversion fast on high-res sensor images.
- `prefetch_display_paths` is called after every navigation with the next 10 images in the folder (see `PREFETCH_COUNT` in `main.js`); these are queued to a single dedicated background thread (`PREFETCH_TX`/`mpsc` channel) rather than converted inline, because concurrent WIC calls across threads are unsafe. On-demand conversion in `get_display_path` itself still runs synchronously on the invoking command thread.
- `cleanup_temp_files` wipes that process's temp directory; it's invoked on Escape/window-close but nothing removes it if the process is killed abnormally.

### Config notes

- `assetProtocol.scope` in `tauri.conf.json` is intentionally wide open (`["**"]`) so `convertFileSrc` can serve both original image paths and the temp HEIC-conversion cache. Don't narrow this without accounting for the temp directory.
- `security.csp` is `null` (disabled).
