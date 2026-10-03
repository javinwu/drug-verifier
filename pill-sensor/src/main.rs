use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use peel::{
    analyze,
    config::ExperimentConfig,
    output::write_report,
    source::{CsvSource, SimulatedSource},
};
use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Peel: optical pill-sensor and dissolution-curve prototype"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a synthetic first-order dissolution experiment (no hardware).
    Simulate {
        #[arg(long, default_value = "config/example.toml")]
        config: PathBuf,
        #[arg(long, default_value = "output/demo")]
        out: PathBuf,
        #[arg(long, default_value_t = 121)]
        samples: u32,
        #[arg(long, default_value_t = 30.0)]
        interval_s: f64,
        #[arg(long, default_value_t = 600.0)]
        time_constant_s: f64,
    },
    /// Analyze sensor CSV readings. Use '-' as input to read stdin until EOF.
    Analyze {
        #[arg(long)]
        input: PathBuf,
        /// Calibration is required so real data never silently uses demo values.
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "output/experiment")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let (points, config, out, simulated) = match Cli::parse().command {
        Command::Simulate {
            config,
            out,
            samples,
            interval_s,
            time_constant_s,
        } => {
            let config = ExperimentConfig::load(&config)?;
            let mut source = SimulatedSource::new(config, samples, interval_s, time_constant_s)?;
            (analyze(&mut source, config)?, config, out, true)
        }
        Command::Analyze { input, config, out } => {
            let config = ExperimentConfig::load(&config)?;
            let reader: Box<dyn Read> = if input.as_os_str() == "-" {
                Box::new(io::stdin())
            } else {
                Box::new(
                    File::open(&input)
                        .with_context(|| format!("cannot open {}", input.display()))?,
                )
            };
            let mut source = CsvSource::new(reader)?;
            (analyze(&mut source, config)?, config, out, false)
        }
    };
    write_report(&out, &points, &config, simulated).context("cannot write experiment report")?;
    let last = points.last().expect("analysis rejects empty experiments");
    let flagged = points
        .iter()
        .filter(|p| !p.quality_flags.is_empty())
        .count();
    println!(
        "Peel | {} | {} readings",
        if simulated {
            "SIMULATED DATA"
        } else {
            "CSV INPUT"
        },
        points.len()
    );
    println!(
        "Final: {:.2}% transmission, {:.4} AU, {:.2}% estimated dissolved at {:.1} min",
        last.transmission_percent,
        last.absorbance_au,
        last.dissolved_percent,
        last.time_s / 60.0
    );
    println!("Quality flags: {flagged} reading(s). See the CSV for details.");
    println!("Curve: {}", out.join("dissolution.svg").display());
    println!("Data:  {}", out.join("dissolution.csv").display());
    Ok(())
}
