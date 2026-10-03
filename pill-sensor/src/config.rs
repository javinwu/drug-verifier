use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Empirical calibration includes the fixed optical path length and wavelength.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentConfig {
    pub slope_au_per_mg_ml: f64,
    pub intercept_au: f64,
    pub vessel_volume_ml: f64,
    pub pill_mass_mg: f64,
}

impl ExperimentConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("cannot read config {}", path.display()))?;
        let config: Self = toml::from_str(&text).context("invalid experiment config")?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("slope_au_per_mg_ml", self.slope_au_per_mg_ml),
            ("vessel_volume_ml", self.vessel_volume_ml),
            ("pill_mass_mg", self.pill_mass_mg),
        ] {
            ensure!(
                value.is_finite() && value > 0.0,
                "{name} must be finite and positive"
            );
        }
        ensure!(self.intercept_au.is_finite(), "intercept_au must be finite");
        Ok(())
    }
}
