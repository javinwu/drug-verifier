use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use peel::{
    analyze,
    config::ExperimentConfig,
    live,
    output::write_report,
    source::{CsvSource, ReadingSource, SimulatedSource},
    stream::{JsonLinesSource, PacedSource, PacketRoundTrip, write_packet},
};
use std::{
    fs::File,
    io::{self, BufReader, Read},
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
    /// Serve an automatically updating graph from fake or external sensor packets.
    Live(LiveArgs),
    /// Act as a fake device: write one JSON packet per reading to stdout.
    Emit {
        #[arg(long, default_value = "config/example.toml")]
        config: PathBuf,
        #[command(flatten)]
        simulation: SimulationArgs,
    },
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

#[derive(Args)]
struct SimulationArgs {
    #[arg(long, default_value_t = 121)]
    samples: u32,
    #[arg(long, default_value_t = 30.0)]
    interval_s: f64,
    #[arg(long, default_value_t = 600.0)]
    time_constant_s: f64,
    /// Experiment seconds per wall-clock second (1 = real time).
    #[arg(long, default_value_t = 60.0)]
    speed: f64,
}

impl SimulationArgs {
    fn source(&self, config: ExperimentConfig) -> Result<PacedSource<SimulatedSource>> {
        PacedSource::new(
            SimulatedSource::new(config, self.samples, self.interval_s, self.time_constant_s)?,
            self.speed,
        )
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum InputFormat {
    Ndjson,
    Csv,
}

#[derive(Args)]
struct LiveArgs {
    /// Required with --input; otherwise uses config/example.toml for simulation.
    #[arg(long)]
    config: Option<PathBuf>,
    /// JSON-lines file, or '-' for an arriving device stream. Omit to simulate.
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "ndjson")]
    format: InputFormat,
    /// Explicitly label a replayed/pipe input as synthetic data.
    #[arg(long, requires = "input")]
    synthetic_input: bool,
    /// Pace recorded input by its timestamps; omit for an already paced device.
    #[arg(long, requires = "input")]
    replay_speed: Option<f64>,
    #[arg(long, default_value = "output/live")]
    out: PathBuf,
    #[arg(long, default_value_t = 8080)]
    port: u16,
    #[command(flatten)]
    simulation: SimulationArgs,
}

fn run_live(args: LiveArgs) -> Result<()> {
    ensure!(
        args.input.is_none() || args.config.is_some(),
        "--input requires an explicit --config calibration"
    );
    let config =
        ExperimentConfig::load(&args.config.unwrap_or_else(|| "config/example.toml".into()))?;
    let simulated = args.input.is_none() || args.synthetic_input;
    let source: Box<dyn ReadingSource + Send> = if let Some(input) = args.input {
        if input.as_os_str() != "-" {
            let input_path = input
                .canonicalize()
                .with_context(|| format!("cannot open {}", input.display()))?;
            for name in [
                "raw.ndjson",
                "dissolution.csv",
                "dissolution.svg",
                "calibration.toml",
                "run.txt",
            ] {
                if let Ok(report_path) = args.out.join(name).canonicalize() {
                    ensure!(
                        input_path != report_path,
                        "input would be overwritten by this run; choose a different --out folder"
                    );
                }
            }
        }
        let reader: Box<dyn Read + Send> = if input.as_os_str() == "-" {
            Box::new(io::stdin())
        } else {
            Box::new(
                File::open(&input).with_context(|| format!("cannot open {}", input.display()))?,
            )
        };
        let source: Box<dyn ReadingSource + Send> = match args.format {
            InputFormat::Ndjson => Box::new(JsonLinesSource::new(BufReader::new(reader))),
            InputFormat::Csv => Box::new(CsvSource::new(reader)?),
        };
        if let Some(speed) = args.replay_speed {
            Box::new(PacedSource::new(source, speed)?)
        } else {
            source
        }
    } else {
        Box::new(PacketRoundTrip(args.simulation.source(config)?))
    };
    live::serve(source, config, args.out, simulated, args.port)
}

fn main() -> Result<()> {
    let (points, config, out, simulated) = match Cli::parse().command {
        Command::Live(args) => return run_live(args),
        Command::Emit { config, simulation } => {
            let config = ExperimentConfig::load(&config)?;
            let mut source = simulation.source(config)?;
            let mut stdout = io::stdout().lock();
            while let Some(reading) = source.next_reading()? {
                write_packet(&mut stdout, &reading)?;
            }
            return Ok(());
        }
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
