# Photo Viewer

A fast, minimal Windows photo viewer built with [Tauri](https://tauri.app/) — meant as a lightweight replacement for the default Windows Photo Viewer. Register it as the handler for JPG/PNG/HEIC files, double-click an image, and it opens straight to that photo with the rest of the folder ready to browse.

## Features

- Opens directly to the double-clicked image, no library or import step
- Arrow through every image in the same folder
- HEIC/HEIF support, transparently decoded via Windows Imaging Component (WIC) and displayed like any other photo
- Background prefetching of upcoming images for snappy navigation
- Delete key moves the current photo to the Recycle Bin (recoverable, not a permanent delete)
- Escape closes the window

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `→` / `↓` | Next image |
| `←` / `↑` | Previous image |
| `Delete` | Move current image to Recycle Bin |
| `Escape` | Close |

## Installation

Grab the latest installer from [Releases](../../releases) and run it — `.msi` or the NSIS `-setup.exe` both work. The installer registers Photo Viewer as an option for JPG, JPEG, PNG, HEIC, and HEIF files; set it as your default via *Settings → Apps → Default apps*, or right-click a file → *Open with* → *Choose another app*.

## Development

Requires [Node.js](https://nodejs.org/) and the [Rust toolchain](https://www.rust-lang.org/tools/install) (plus the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows).

```sh
npm install
npm run tauri dev
```

Other commands:

- `npm run dev` — Vite dev server only (frontend hot-reload in a browser tab; no Tauri window)
- `npm run build` — Vite production build only, writes `dist/`
- `npm run tauri build` — full release build; produces NSIS and MSI installers under `src-tauri/target/release/bundle/`

> File associations are only written to the Windows registry when a generated installer is *run* — building or running in dev mode doesn't touch them. See `CLAUDE.md` for more on the architecture and this caveat.

## License

No license specified yet.
