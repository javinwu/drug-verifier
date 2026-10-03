use crate::{config::ExperimentConfig, pill_sensor::RawReading};
use anyhow::{Context, Result, ensure};
use std::io::Read;

/// Implement this trait for a USB, serial, ADC, or other physical sensor.
/// `None` means the experiment ended; an error means acquisition failed.
pub trait ReadingSource {
    fn next_reading(&mut self) -> Result<Option<RawReading>>;
}

/// CSV also works with a finite sensor stream supplied through stdin.
pub struct CsvSource<R: Read> {
    reader: csv::Reader<R>,
}

impl<R: Read> CsvSource<R> {
    pub fn new(reader: R) -> Result<Self> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(reader);
        let expected = [
            "time_s",
            "reference_intensity",
            "sample_intensity",
            "dark_intensity",
        ];
        let headers = reader.headers().context("cannot read CSV header")?;
        ensure!(
            headers.len() == expected.len()
                && expected
                    .iter()
                    .all(|h| headers.iter().filter(|v| v == h).count() == 1),
            "CSV must contain exactly these headers: {}",
            expected.join(",")
        );
        Ok(Self { reader })
    }
}

impl<R: Read> ReadingSource for CsvSource<R> {
    fn next_reading(&mut self) -> Result<Option<RawReading>> {
        self.reader
            .deserialize()
            .next()
            .transpose()
            .context("invalid sensor CSV row")
    }
}

/// Deterministic synthetic first-order dissolution, never a hardware measurement.
pub struct SimulatedSource {
    config: ExperimentConfig,
    index: u32,
    samples: u32,
    interval_s: f64,
    time_constant_s: f64,
}

impl SimulatedSource {
    pub fn new(
        config: ExperimentConfig,
        samples: u32,
        interval_s: f64,
        time_constant_s: f64,
    ) -> Result<Self> {
        config.validate()?;
        ensure!(samples >= 2, "simulation needs at least two samples");
        ensure!(
            interval_s.is_finite() && interval_s > 0.0,
            "interval must be finite and positive"
        );
        ensure!(
            time_constant_s.is_finite() && time_constant_s > 0.0,
            "time constant must be finite and positive"
        );
        ensure!(
            (f64::from(samples - 1) * interval_s).is_finite(),
            "simulation duration overflows"
        );
        Ok(Self {
            config,
            index: 0,
            samples,
            interval_s,
            time_constant_s,
        })
    }
}

impl ReadingSource for SimulatedSource {
    fn next_reading(&mut self) -> Result<Option<RawReading>> {
        if self.index >= self.samples {
            return Ok(None);
        }
        let time_s = f64::from(self.index) * self.interval_s;
        let fraction = -(-time_s / self.time_constant_s).exp_m1();
        let concentration = fraction * self.config.pill_mass_mg / self.config.vessel_volume_ml;
        let absorbance = self.config.intercept_au + self.config.slope_au_per_mg_ml * concentration;
        let reading = RawReading {
            time_s,
            reference_intensity: 4010.0,
            sample_intensity: 10.0 + 4000.0 * 10.0_f64.powf(-absorbance),
            dark_intensity: 10.0,
        };
        self.index += 1;
        Ok(Some(reading))
    }
}
