# Hardware-less live sensor pipeline

All commands below run from `pill-sensor/`. Existing `simulate` and `analyze`
commands still generate batch reports. The live path builds on their
`ReadingSource`, `PillSensor::process`, calibration, and SVG/CSV exporter.

```text
fake sensor / recorded readings / future ESP32 serial bridge
                         ↓
         newline-delimited JSON (or existing CSV)
                         ↓
         ReadingSource → RawReading → PillSensor::process
                         ↓
          live SVG + CSV + packet log + JSON status
                         ↓
              local browser / future application
```

## One-command demo

```bash
cargo run --locked -- live
```

Open **http://127.0.0.1:8080**. The default is 121 readings over 60 experiment
minutes, a 10-minute first-order time constant, and 60× playback. One reading
arrives about every 0.5 wall-clock seconds, so the run takes about one minute.
The first reading arrives immediately. The graph and CSV are rewritten after
each valid point; the browser checks for updates every 500 ms.

The built-in fake device uses the existing deterministic simulator and encodes
and decodes every reading through the JSON protocol in-process. These are ideal
synthetic signals, not a model of a specific drug, sensor noise, bubbles, or ADC
saturation. Calibration values in the included configs are invented.

```bash
# Use the existing fictional ibuprofen configuration.
cargo run --locked -- live --config config/ibuprofen_demo.toml

# Slower playback or a different synthetic curve.
cargo run --locked -- live --samples 61 --interval-s 60 --time-constant-s 900 --speed 30 --port 8081 --out output/live-test
```

`--speed 1` uses real-time measurement intervals. Speed changes the wait between
packets, never their experiment timestamps or calculated results. Acquisition
starts when the command starts; refreshing the browser does not restart it.
The server remains open after completion or an acquisition error. Press Ctrl+C
to stop the process; rerun the command to start a new experiment.

## Exercise a separate fake device

Build once so two Cargo processes do not compete for the build lock:

```bash
cargo build --locked
./target/debug/peel emit --config config/ibuprofen_demo.toml | ./target/debug/peel live --input - --config config/ibuprofen_demo.toml --synthetic-input
```

`emit` flushes one JSON packet per reading to stdout, with errors on stderr.
`live --input -` processes packets as they arrive, without additional pacing.
Replace the command to the left of the pipe with a future serial bridge. The
receiver, calculations, report, and UI remain the same. Use `--synthetic-input`
only for fake input; external input otherwise carries an external-source label.
An explicit `--config` is required for all external/replayed input.

## Replay existing files

```bash
# Reuse the existing ibuprofen CSV; no extra drug datasets are needed.
cargo run --locked -- live --input data/ibuprofen_synthetic.csv --format csv --config config/ibuprofen_demo.toml --synthetic-input --replay-speed 60 --out output/ibuprofen-live

# Replay a packet log from a prior default-config run into a different folder.
cargo run --locked -- live --input output/live/raw.ndjson --config output/live/calibration.toml --synthetic-input --replay-speed 60 --out output/replay
```

Without `--replay-speed`, a file is consumed as fast as possible. For an already
paced input stream, omit it. Use the same calibration that produced the original
run. Never replay into the folder containing the input: live reports overwrite
their destination. The CLI rejects replay input that would itself be overwritten.

## Device packet contract

One UTF-8 JSON object per line, terminated by `\n` (`\r\n` also works):

```json
{"time_s":60.0,"reference_intensity":4010.0,"sample_intensity":3187.31,"dark_intensity":10.0}
```

- `time_s`: elapsed experiment seconds, nonnegative and strictly increasing.
- Intensities: linear detector values in matching units/gain; reference and
  sample must exceed the common dark background.
- Exactly these four fields are accepted. Each packet is limited to 4096 bytes
  including its newline. Missing, duplicate, unknown, or nonnumeric fields fail.
- A partial packet waits for its newline. EOF mid-packet, malformed JSON,
  invalid optical signals, or backwards/duplicate timestamps stops acquisition.
- EOF between packets means complete. An empty experiment is an error. A pause
  without EOF simply waits; there is no invented reading or automatic completion.
- Quality flags are preserved; implausible but calculable values are not clamped.

The processor performs dark correction, transmittance, absorbance, empirical
concentration calibration, and mass/percent estimation in Rust. No dissolution
percentage is supplied by the fake device, and the browser does no sensor math.

## Reports and API

`output/live/` contains `dissolution.svg`, `dissolution.csv`, `calibration.toml`,
`run.txt`, and `raw.ndjson`. Raw packets that parse successfully are logged before
processing, including a packet whose optical values subsequently fail validation.
Malformed text is reported as an error rather than silently skipped. The graph
and results contain only processed points. `run.txt` records whether the run is
running, complete, or failed. If interrupted by Ctrl+C it retains the last state
(usually `running`), not a false completion marker.

The local server binds to `127.0.0.1` only:

| Endpoint | Content |
| --- | --- |
| `/` | Browser dashboard |
| `/api/status` | Run ID, waiting/running/complete/error status, source, reading count, latest processed point, flag count, error, and calibration |
| `/curve.svg` | Current graph from the existing Rust renderer; 404 before the first point |
| `/dissolution.csv` | Current computed results |
| `/raw.ndjson` | Recorded sensor packets |
| `/calibration.toml` | Calibration used |
| `/run.txt` | Run state and assumptions |

Responses disable caching. The browser retries after disconnects and detects
new run IDs when the server restarts. Each SVG/CSV snapshot is written to a
temporary file and renamed, so readers cannot see a half-written snapshot.
Separate files are not a transactional bundle; wait for `complete` when exporting
a final report. Use a separate output folder and port for concurrent experiments.

This prototype retains the experiment in memory and rewrites the report for
every reading. It suits small, finite experiments like the default 121 points.
Long-running/high-rate acquisition will need bounded storage and incremental
history retrieval. No firmware, USB driver, Wi-Fi ingestion, database, or deployed
service is required or provided here.

## Future app decision

Keep the live curve in the development view. A future app can use the status API
for a simple measurement summary and offer the curve under “View details.”
The current pipeline estimates dissolution; it does not identify a drug, compare
against validated reference profiles, or determine pill authenticity. A future
reference-comparison result needs measured reference data and a validated model
before it can support a user-facing claim.
