# Peel

Peel MIT hackathon project.

```text
pill-sensor/   Rust sensor analysis, simulations, calibration, data, and tests
app/           Tauri desktop application and TypeScript frontend
.github/       Repository CI workflows
```

## Pill sensor

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

The [app folder](app/README.md) contains the Tauri desktop application and TypeScript frontend. See its README for setup and development commands.
