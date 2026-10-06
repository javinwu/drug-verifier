# Peel pill sensor

A Rust starter for the Peel MIT hackathon project: turn optical pill-sensor readings into transmission, absorbance, and an estimated dissolution curve.

The project runs immediately with simulated data or CSV recordings. Physical sensor acquisition is an extension point; no hardware driver is included yet.

## Run the demo

For an automatically updating browser graph, run `cargo run --locked -- live`
from `pill-sensor/` and open http://127.0.0.1:8080. This streams simulated sensor
packets through the existing calculations and updates `output/live/` after each
valid reading. See the [live pipeline guide](docs/live.md) for pacing, JSON
packets, CSV replay, and the future ESP32 boundary. The batch commands below
continue to work as before.

Install stable Rust with Cargo, then enter `pill-sensor/` from the repository root. Run all commands in this guide from that folder:

```bash
cd pill-sensor
```

```bash
cargo run --locked -- simulate
```

Open `output/demo/dissolution.svg` in a browser to see transmission, absorbance, and estimated percent dissolved over time. The demo generates 121 synthetic readings over 60 minutes, with a 10-minute first-order time constant. It should finish at about **99.75% estimated dissolved**.

```bash
# Customize the synthetic experiment.
cargo run --locked -- simulate --samples 61 --interval-s 30 --time-constant-s 300

# Analyze the included synthetic CSV fixture.
cargo run --locked -- analyze --input data/example_readings.csv --config config/example.toml

# Analyze a finite sensor stream; the report is written when stdin reaches EOF.
cat data/example_readings.csv | cargo run --locked -- analyze --input - --config config/example.toml

cargo run --locked -- --help
```

Each run writes `dissolution.csv` (raw signals, calculations, and quality flags), `dissolution.svg` (three plots), `calibration.toml` (the configuration used), and `run.txt` (input mode and assumptions). Pass `--out output/my-experiment` to choose a destination. Reusing a destination overwrites these report files.

## Begin with the pill sensor

### Fictional ibuprofen example

`data/ibuprofen_synthetic.csv` is a single dataset with 61 fake readings for a
fictional 200 mg ibuprofen experiment over 60 minutes. The curve and optical
calibration are invented demo assumptions, not measured ibuprofen properties
or a prediction of how a real tablet dissolves. This uses the original demo's
10-minute first-order time constant and an assumed vessel volume of 900 mL.

From `pill-sensor/`, generate the graph:

```bash
cargo run --locked -- analyze --input data/ibuprofen_synthetic.csv --config config/ibuprofen_demo.toml --out output/ibuprofen
```

Open `output/ibuprofen/dissolution.svg` in a browser. The report says
"CSV SENSOR INPUT" because it was loaded from a file; this fixture is synthetic.
The assumed curve reaches about 63.21% at 10 minutes, 95.02% at 30 minutes, and
99.75% at 60 minutes. Calculated values are in `output/ibuprofen/dissolution.csv`.
Re-running the command replaces the same report.

### Processing readings

The starting implementation is **[`src/pill_sensor.rs`](src/pill_sensor.rs)**. `PillSensor::process` accepts one timestamped `RawReading` and returns a `DissolutionPoint`.

```text
sensor / CSV / simulation
          ↓
      RawReading
          ↓
  dark-signal correction
          ↓
 transmission → absorbance
          ↓
 empirical calibration → concentration → dissolved mass / percentage
          ↓
      CSV + SVG curve
```

CSV input must contain exactly these columns (column order can vary):

```csv
time_s,reference_intensity,sample_intensity,dark_intensity
0,4010,4010,10
60,4010,410,10
```

- `time_s`: elapsed seconds since the experiment began; strictly increasing.
- `reference_intensity`: matched solvent blank/reference signal before sample attenuation.
- `sample_intensity`: light detected through the sample.
- `dark_intensity`: detector background with the light source off.

Intensities are nonnegative, linear detector readings in matching units and gain settings. Reference and sample must both exceed the dark signal. The simple model assumes a shared detector background; separate detectors may require separate corrections before creating a reading.

## Calculations and calibration

```text
T = (sample - dark) / (reference - dark)
transmission_percent = 100 × T
A = -log10(T)
concentration_mg_ml = (A - intercept_au) / slope_au_per_mg_ml
dissolved_mass_mg = concentration_mg_ml × vessel_volume_ml
dissolved_percent = 100 × dissolved_mass_mg / pill_mass_mg
```

The optical definitions follow [IUPAC transmittance](https://goldbook.iupac.org/terms/view/T06484) and [Beer–Lambert law](https://goldbook.iupac.org/terms/view/B00626). An empirical linear calibration maps absorbance to concentration at a fixed wavelength and optical path. Absorbance is logarithmic; it is not a percentage of light absorbed.

`config/example.toml` contains **invented demonstration values**, and `data/example_readings.csv` is synthetic. For measured experiments, copy the configuration and replace its values using concentration standards collected with the same wavelength, path length, solvent, and sensor settings. `pill_mass_mg` is the initial mass of the absorbing active ingredient, not the tablet's total mass. `analyze` requires an explicit `--config` argument.

The concentration-to-mass estimate assumes a well-mixed solution, a constant vessel volume, a selective linear optical response, and no withdrawn samples or additional dilution. Scattering, bubbles, excipients, saturation, or changing geometry can invalidate the estimate. This is an experimental prototype, not a validated pharmaceutical dissolution method.

Readings above 100% transmission, negative concentrations, and dissolution above 100% remain in the output with quality flags. The program does not clamp, smooth, or force the curve to increase. Missing/invalid signals, nonfinite calculations, and out-of-order timestamps stop analysis with an error before report writing. The SVG simply connects measured points.

## Project files

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Command-line entry point |
| `src/lib.rs` | Reusable experiment runner |
| `src/pill_sensor.rs` | Sensor processing and dissolution calculations |
| `src/source.rs` | Acquisition trait, CSV reader, simulator |
| `src/stream.rs` | JSON device packets and paced playback |
| `src/live.rs` | Incremental reports, live status API, and local server |
| `src/config.rs` | Calibration loading and validation |
| `src/output.rs` | CSV, SVG, and experiment record export |
| `config/example.toml` | Demo calibration and experiment settings |
| `data/example_readings.csv` | Synthetic input fixture |
| `docs/hardware.md` | Hardware integration starting point |
| `docs/live.md` | Live simulation, replay, and API guide |
| `../app/live/` | Live browser dashboard served by Rust |
| `tests/` | Scientific calculation, input-validation, and CLI tests |
| `../.github/workflows/ci.yml` | Formatting, lint, and test checks |

## Development

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

`Cargo.lock` is included with the starter for reproducible dependency versions. No API keys, services, Python, or plotting software are required. Cargo downloads dependencies on the first build.
