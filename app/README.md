# Peel app

The Peel application uses Tauri with vanilla HTML, CSS, and TypeScript.

From this folder, run:

```bash
npm install
npm run tauri dev
```

Desktop development requires Rust and the Tauri platform prerequisites. Use
`npm run dev` to run the frontend in a browser, `npm run build` to build the
frontend, or `npm run tauri build` to build the desktop application.

Frontend files live in `src/` and `index.html`; desktop files live in
`src-tauri/`. The Rust sensor project lives in
[`../pill-sensor/`](../pill-sensor/README.md).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Live sensor dashboard

The dashboard in `live/` uses plain HTML, CSS, and JavaScript. Rust embeds these
files at compile time and serves them; no Node build, frontend framework, or
external chart service is needed.

From the repository root:

```bash
cd pill-sensor
cargo run --locked -- live
```

Open http://127.0.0.1:8080. Changes to these files require rebuilding/restarting
the Rust command. The page polls `/api/status` every 500 ms and loads `/curve.svg`
when new points arrive. The SVG comes from the existing Rust renderer; JavaScript
does not recalculate the optical or dissolution values.

The page shows acquisition state, latest raw/processed readings, quality flags,
and downloads. It retries after disconnections and preserves the last displayed
curve. A new server run resets the display. See the
[pipeline guide](../pill-sensor/docs/live.md) for commands and API details.

For the future app, keep a simple experiment summary as the primary view and
offer the curve as a detail view. This prototype has no pill identification,
reference classifier, or authenticity decision model.
