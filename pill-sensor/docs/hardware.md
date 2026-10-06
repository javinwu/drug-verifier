# Connecting a physical pill sensor

The acquisition boundary is `ReadingSource` in `src/source.rs`:

```rust
pub trait ReadingSource {
    fn next_reading(&mut self) -> anyhow::Result<Option<RawReading>>;
}
```

Implement it for the selected microcontroller, serial device, ADC, or vendor SDK. Return `Some(reading)` for a completed measurement, `None` only when the experiment ends, and `Err` on acquisition failure. A temporary timeout should not silently end the experiment.

The required hardware choices are the detector interface, intensity scale, wavelength, optical path, gain/exposure, and reference measurement method. The starter deliberately does not guess a board pinout, baud rate, or sensor register map.

## First integration

1. Measure the dark background with illumination off.
2. Measure a matched blank at the experiment's wavelength and geometry.
3. Record known concentration standards and fit `A = slope × concentration + intercept` over the intended measurement range.
4. Put the slope, intercept, constant vessel volume, and active ingredient mass into a new TOML configuration.
5. Acquire timestamped sample readings with the same settings. Send the four documented columns as CSV, or implement `ReadingSource` for the device and pass it to `peel::analyze`.
6. Inspect the raw readings and quality flags alongside the curve. Compare measured standards with their known concentrations before interpreting pill runs.

The proposed streaming protocol is one newline-delimited JSON object per
measurement, using the same four fields as `RawReading`. A host serial bridge
can forward device bytes to `peel live --input - --config your-calibration.toml`.
Each complete packet updates the report and live browser graph immediately.
See the [live pipeline guide](live.md) for the packet contract and a fake-device
command that exercises this handoff without hardware. A pause in incoming data
does not end acquisition; EOF ends the experiment, and malformed input stops it
with a visible error while preserving the last valid curve.

Firmware may instead emit a CSV header once and then one row per measurement;
use `peel live --input - --format csv --config your-calibration.toml` for this
format. The original `peel analyze --input - --config your-calibration.toml`
still waits for a finite capture to reach EOF before exporting a batch report.

The live runner calls `PillSensor::process` for every reading, exactly as batch
analysis does. The simulator, recorded files, and future device therefore share
one analysis implementation. Direct serial/USB access and firmware are not yet
implemented; the host bridge supplies the bytes on stdin.

## Assumptions to revisit with real hardware

- A common dark value is appropriate for the matched sample/reference readings. Independent detectors may have different offsets and responses.
- Raw readings are linear in optical intensity. Convert logarithmic or otherwise encoded sensor outputs before processing.
- Firmware checks device-specific saturation and detection limits; the starter does not know the ADC range or sensitivity.
- The selected wavelength detects the dissolved analyte with a usable calibration. A sensor cannot distinguish absorption from scattering using this ratio alone.
- The solution stays well mixed at constant volume. Sampling withdrawals, replacement medium, and dilution require additional mass-balance logic.

The simulator produces ideal first-order data solely to exercise the software. It does not predict a particular pill's behavior.
