# Peel app

The local live dashboard uses plain HTML, CSS, and JavaScript. Rust embeds these
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
