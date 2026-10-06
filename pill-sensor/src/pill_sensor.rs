//! Start here when extending the pill sensor.
//!
//! Acquisition supplies a sample signal, matched blank/reference, and dark signal.
//! All three signals must use the same linear intensity scale and detector gain.
//! This module derives optical values; it does not control a physical detector.
use crate::config::ExperimentConfig;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawReading {
    pub time_s: f64,
    pub reference_intensity: f64,
    pub sample_intensity: f64,
    pub dark_intensity: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DissolutionPoint {
    pub time_s: f64,
    pub reference_intensity: f64,
    pub sample_intensity: f64,
    pub dark_intensity: f64,
    pub transmittance: f64,
    pub transmission_percent: f64,
    pub absorbance_au: f64,
    pub concentration_mg_ml: f64,
    pub dissolved_mass_mg: f64,
    pub dissolved_percent: f64,
    pub quality_flags: String,
}

pub struct PillSensor {
    config: ExperimentConfig,
    previous_time_s: Option<f64>,
}

impl PillSensor {
    pub fn new(config: ExperimentConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            previous_time_s: None,
        })
    }

    pub fn process(&mut self, reading: RawReading) -> Result<DissolutionPoint> {
        let RawReading {
            time_s,
            reference_intensity,
            sample_intensity,
            dark_intensity,
        } = reading;
        for (name, value) in [
            ("time_s", time_s),
            ("reference_intensity", reference_intensity),
            ("sample_intensity", sample_intensity),
            ("dark_intensity", dark_intensity),
        ] {
            ensure!(
                value.is_finite() && value >= 0.0,
                "{name} must be finite and nonnegative at t={time_s}"
            );
        }
        if let Some(previous) = self.previous_time_s {
            ensure!(
                time_s > previous,
                "timestamps must strictly increase: {time_s} follows {previous}"
            );
        }
        let reference = reference_intensity - dark_intensity;
        let sample = sample_intensity - dark_intensity;
        ensure!(
            reference > 0.0,
            "reference must exceed dark signal at t={time_s}"
        );
        ensure!(
            sample > 0.0,
            "sample must exceed dark signal at t={time_s}; absorbance is undefined"
        );

        let transmittance = sample / reference;
        let transmission_percent = 100.0 * transmittance;
        let absorbance_au = -transmittance.log10();
        let concentration_mg_ml =
            (absorbance_au - self.config.intercept_au) / self.config.slope_au_per_mg_ml;
        let dissolved_mass_mg = concentration_mg_ml * self.config.vessel_volume_ml;
        let dissolved_percent = dissolved_mass_mg / self.config.pill_mass_mg * 100.0;
        ensure!(
            [
                transmittance,
                transmission_percent,
                absorbance_au,
                concentration_mg_ml,
                dissolved_mass_mg,
                dissolved_percent
            ]
            .iter()
            .all(|v| v.is_finite()),
            "calculation overflow or underflow at t={time_s}; check signals and calibration"
        );

        // Preserve measured values instead of hiding calibration problems by clamping.
        let mut flags = Vec::new();
        if transmittance > 1.0 {
            flags.push("transmission_above_100_percent");
        }
        if concentration_mg_ml < 0.0 {
            flags.push("negative_concentration");
        }
        if dissolved_percent > 100.0 {
            flags.push("dissolution_above_100_percent");
        }
        self.previous_time_s = Some(time_s);
        Ok(DissolutionPoint {
            time_s,
            reference_intensity,
            sample_intensity,
            dark_intensity,
            transmittance,
            transmission_percent,
            absorbance_au,
            concentration_mg_ml,
            dissolved_mass_mg,
            dissolved_percent,
            quality_flags: flags.join(";"),
        })
    }
}
