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
