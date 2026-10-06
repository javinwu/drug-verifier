# Peel

Peel MIT hackathon project.

```text
pill-sensor/   Rust sensor analysis, simulations, calibration, data, and tests
app/           Local live dashboard, served by the Rust program
.github/       Repository CI workflows
```

## Pill sensor

Start the hardware-less live sensor pipeline from the repository root:

```bash
cd pill-sensor
cargo run --locked -- live
```

Open **http://127.0.0.1:8080**. The fake device sends a reading every half second,
replaying an hour of experiment time in about a minute. The graph updates
automatically; the server stays open after completion. Press Ctrl+C to stop it.
See the [live pipeline guide](pill-sensor/docs/live.md) for device packets,
CSV replay, and future ESP32 integration.

The Rust prototype converts simulated or CSV optical readings into transmission, absorbance, and estimated dissolution curves.

From the repository root:

```bash
cd pill-sensor
cargo run --locked -- simulate
```

Open `pill-sensor/output/demo/dissolution.svg` from the repository root, or `output/demo/dissolution.svg` from inside `pill-sensor/`.

Run tests from `pill-sensor/`:

```bash
cargo test --locked
```

See the [pill sensor guide](pill-sensor/README.md) for CSV inputs, calibration, and custom experiments, and the [hardware guide](pill-sensor/docs/hardware.md) for sensor integration.

## App

The [app folder](app/README.md) contains a lightweight browser dashboard embedded
in the Rust server. The JSON status API keeps acquisition and analysis separate
from the UI, so a future app can consume the same results.
