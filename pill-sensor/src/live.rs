//! Incremental acquisition uses the same processor and SVG exporter as batch runs.
use crate::{
    config::ExperimentConfig,
    output::{ReportSource, render_svg_with_source, write_atomic, write_report_with_source},
    pill_sensor::{DissolutionPoint, PillSensor},
    source::ReadingSource,
    stream::write_packet,
};
use anyhow::{Context, Result, anyhow, ensure};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tiny_http::{Header, Method, Response, Server};

#[derive(Clone, Serialize)]
pub struct LiveSnapshot {
    pub run_id: String,
    pub status: &'static str,
    pub source: &'static str,
    pub count: usize,
    pub flagged: usize,
    pub latest: Option<DissolutionPoint>,
    pub error: Option<String>,
    pub config: ExperimentConfig,
    #[serde(skip)]
    pub svg: Option<String>,
}

impl LiveSnapshot {
    pub fn new(config: ExperimentConfig, simulated: bool) -> Self {
        Self {
            run_id: format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            status: "waiting",
            source: if simulated {
                "SIMULATED DATA"
            } else {
                "EXTERNAL SENSOR STREAM"
            },
            count: 0,
            flagged: 0,
            latest: None,
            error: None,
            config,
            svg: None,
        }
    }
}

/// Publish only after the point and its report have been successfully saved.
pub fn collect_live(
    source: &mut dyn ReadingSource,
    config: ExperimentConfig,
    out: &Path,
    simulated: bool,
    mut publish: impl FnMut(LiveSnapshot),
) -> Result<()> {
    let mut snapshot = LiveSnapshot::new(config, simulated);
    let report_source = if simulated {
        ReportSource::Simulated
    } else {
        ReportSource::Stream
    };
    let mut points = Vec::new();
    let result: Result<()> = (|| {
        let mut sensor = PillSensor::new(config)?;
        fs::create_dir_all(out)?;
        // Clear this run's reports immediately so an empty/failed stream cannot
        // appear to have produced a previous run's results.
        write_atomic(&out.join("dissolution.csv"), b"")?;
        write_atomic(&out.join("dissolution.svg"), b"")?;
        write_atomic(
            &out.join("calibration.toml"),
            toml::to_string_pretty(&config)?.as_bytes(),
        )?;
        write_atomic(
            &out.join("run.txt"),
            format!("{}\nLive status: waiting\n", snapshot.source).as_bytes(),
        )?;
        let mut raw = File::create(out.join("raw.ndjson"))?;
        while let Some(reading) = source.next_reading()? {
            write_packet(&mut raw, &reading).context("cannot record sensor packet")?;
            let point = sensor.process(reading)?;
            points.push(point.clone());
            write_report_with_source(out, &points, &config, report_source)
                .context("cannot save live report")?;
            snapshot.svg = Some(render_svg_with_source(&points, report_source)?);
            snapshot.count = points.len();
            snapshot.flagged += usize::from(!point.quality_flags.is_empty());
            snapshot.latest = Some(point);
            snapshot.status = "running";
            save_status(out, &snapshot)?;
            publish(snapshot.clone());
        }
        ensure!(!points.is_empty(), "input contains no sensor readings");
        Ok(())
    })();
    match &result {
        Ok(()) => snapshot.status = "complete",
        Err(error) => {
            snapshot.status = "error";
            snapshot.error = Some(format!("{error:#}"));
        }
    }
    // Retain the last valid curve on failure and mark its report as partial.
    let saved = save_status(out, &snapshot);
    if let Err(error) = &saved {
        snapshot.status = "error";
        let previous = snapshot
            .error
            .take()
            .map(|s| format!("{s}; "))
            .unwrap_or_default();
        snapshot.error = Some(format!("{previous}cannot save run status: {error:#}"));
    }
    publish(snapshot);
    result.and(saved)
}

fn save_status(out: &Path, snapshot: &LiveSnapshot) -> Result<()> {
    let record = format!(
        "Peel live | {}\nLive status: {}\n{} readings; {} flagged\n{}\nDissolution estimates use calibration.toml.\nConstant vessel volume; no withdrawn samples or dilution correction.\n",
        snapshot.source,
        snapshot.status,
        snapshot.count,
        snapshot.flagged,
        snapshot.error.as_deref().unwrap_or("")
    );
    write_atomic(&out.join("run.txt"), record.as_bytes())
}

/// The HTTP listener remains available after EOF so users can inspect the result.
pub fn serve(
    mut source: Box<dyn ReadingSource + Send>,
    config: ExperimentConfig,
    out: PathBuf,
    simulated: bool,
    port: u16,
) -> Result<()> {
    let server =
        Server::http(("127.0.0.1", port)).map_err(|e| anyhow!("cannot start live server: {e}"))?;
    let state = Arc::new(Mutex::new(LiveSnapshot::new(config, simulated)));
    let worker_state = Arc::clone(&state);
    let worker_out = out.clone();
    thread::spawn(move || {
        if let Err(error) = collect_live(&mut *source, config, &worker_out, simulated, |snapshot| {
            *worker_state.lock().expect("live state mutex poisoned") = snapshot;
        }) {
            eprintln!("Acquisition stopped: {error:#}");
        }
    });
    println!("Peel live: http://{}", server.server_addr());
    println!("Reports: {} | Ctrl+C to stop the server", out.display());
    std::io::stdout().flush()?;

    for request in server.incoming_requests() {
        let path = request.url().split('?').next().unwrap_or("/");
        let (code, mime, body) = if request.method() != &Method::Get {
            (405, "text/plain", b"Use GET".to_vec())
        } else {
            match path {
                "/" => (
                    200,
                    "text/html; charset=utf-8",
                    include_bytes!("../../app/index.html").to_vec(),
                ),
                "/app.js" => (
                    200,
                    "text/javascript; charset=utf-8",
                    include_bytes!("../../app/app.js").to_vec(),
                ),
                "/style.css" => (
                    200,
                    "text/css; charset=utf-8",
                    include_bytes!("../../app/style.css").to_vec(),
                ),
                "/api/status" => (
                    200,
                    "application/json",
                    serde_json::to_vec(&*state.lock().expect("live state mutex poisoned"))?,
                ),
                "/curve.svg" => match &state.lock().expect("live state mutex poisoned").svg {
                    Some(svg) => (200, "image/svg+xml", svg.as_bytes().to_vec()),
                    None => (
                        404,
                        "text/plain",
                        b"Waiting for the first valid reading".to_vec(),
                    ),
                },
                "/dissolution.csv" | "/raw.ndjson" | "/calibration.toml" | "/run.txt" => {
                    match fs::read(out.join(path.trim_start_matches('/'))) {
                        Ok(body) => (200, "text/plain; charset=utf-8", body),
                        Err(_) => (404, "text/plain", b"Report not available yet".to_vec()),
                    }
                }
                _ => (404, "text/plain", b"Not found".to_vec()),
            }
        };
        let response = Response::from_data(body)
            .with_status_code(code)
            .with_header(Header::from_bytes("Content-Type", mime).unwrap())
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
        // Closing a browser tab must not terminate acquisition or other clients.
        let _ = request.respond(response);
    }
    Ok(())
}
