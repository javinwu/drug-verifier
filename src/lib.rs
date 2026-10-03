//! Optical sensor acquisition and calibrated dissolution estimates for Peel.
pub mod config;
pub mod output;
pub mod pill_sensor;
pub mod source;

use anyhow::{Result, ensure};
use config::ExperimentConfig;
use pill_sensor::{DissolutionPoint, PillSensor};
use source::ReadingSource;

/// Analyze a complete experiment. Invalid readings fail the run, never disappear.
pub fn analyze(
    source: &mut impl ReadingSource,
    config: ExperimentConfig,
) -> Result<Vec<DissolutionPoint>> {
    let mut sensor = PillSensor::new(config)?;
    let mut points = Vec::new();
    while let Some(reading) = source.next_reading()? {
        points.push(sensor.process(reading)?);
    }
    ensure!(!points.is_empty(), "input contains no sensor readings");
    Ok(points)
}
