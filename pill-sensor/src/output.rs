use crate::{config::ExperimentConfig, pill_sensor::DissolutionPoint};
use anyhow::{Result, ensure};
use std::{fmt::Write as _, fs, path::Path};

#[derive(Clone, Copy)]
pub enum ReportSource {
    Simulated,
    Csv,
    Stream,
}

impl ReportSource {
    fn label(self) -> &'static str {
        match self {
            Self::Simulated => "SIMULATED DATA",
            Self::Csv => "CSV SENSOR INPUT",
            Self::Stream => "EXTERNAL SENSOR STREAM",
        }
    }
}

/// Export raw and derived data together, calibration, and a standalone SVG plot.
pub fn write_report(
    directory: &Path,
    points: &[DissolutionPoint],
    config: &ExperimentConfig,
    simulated: bool,
) -> Result<()> {
    write_report_with_source(
        directory,
        points,
        config,
        if simulated {
            ReportSource::Simulated
        } else {
            ReportSource::Csv
        },
    )
}

pub fn write_report_with_source(
    directory: &Path,
    points: &[DissolutionPoint],
    config: &ExperimentConfig,
    source: ReportSource,
) -> Result<()> {
    ensure!(!points.is_empty(), "cannot export an empty experiment");
    // Build the plot first so invalid data never produces a partial CSV report.
    let svg = render_svg_with_source(points, source)?;
    fs::create_dir_all(directory)?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    for point in points {
        writer.serialize(point)?;
    }
    let csv = writer.into_inner()?;
    write_atomic(&directory.join("dissolution.csv"), &csv)?;
    write_atomic(&directory.join("dissolution.svg"), svg.as_bytes())?;
    write_atomic(
        &directory.join("calibration.toml"),
        toml::to_string_pretty(config)?.as_bytes(),
    )?;
    let source = source.label();
    write_atomic(
        &directory.join("run.txt"),
        format!(
            "Peel {} | {source}\n{} readings\nDissolution estimates use calibration.toml.\nConstant vessel volume; no withdrawn samples or dilution correction.\n",
            env!("CARGO_PKG_VERSION"),
            points.len()
        ).as_bytes(),
    )?;
    Ok(())
}

/// Readers see a complete previous or new file, never a partly written graph.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("report")
    ));
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)?;
    Ok(())
}

/// Raw points are joined by straight lines; no smoothing or kinetic fit is applied.
pub fn render_svg(points: &[DissolutionPoint], simulated: bool) -> Result<String> {
    render_svg_with_source(
        points,
        if simulated {
            ReportSource::Simulated
        } else {
            ReportSource::Csv
        },
    )
}

pub fn render_svg_with_source(points: &[DissolutionPoint], source: ReportSource) -> Result<String> {
    ensure!(!points.is_empty(), "cannot plot an empty experiment");
    for (index, point) in points.iter().enumerate() {
        ensure!(
            [
                point.time_s,
                point.transmission_percent,
                point.absorbance_au,
                point.dissolved_percent
            ]
            .iter()
            .all(|v| v.is_finite()),
            "cannot plot nonfinite values"
        );
        if index > 0 {
            ensure!(
                point.time_s > points[index - 1].time_s,
                "plot timestamps must increase"
            );
        }
    }
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="940" viewBox="0 0 1000 940" role="img" aria-labelledby="title desc">
<title id="title">Peel pill dissolution experiment</title>
<desc id="desc">Transmission, absorbance, and calibrated percent dissolved plotted against elapsed time.</desc>
<rect width="1000" height="940" fill="#f5f5ef"/>
<g font-family="system-ui, sans-serif" fill="#163c32">
<text x="80" y="48" font-size="30" font-weight="700">peel / pill dissolution</text>
"##,
    );
    let source = source.label();
    let flagged = points
        .iter()
        .filter(|p| !p.quality_flags.is_empty())
        .count();
    writeln!(
        svg,
        r#"<text x="80" y="77" font-size="14">{source} · {} readings · {flagged} flagged · calibrated estimates</text>"#,
        points.len()
    )?;
    let start = points[0].time_s;
    let span = (points.last().unwrap().time_s - start).max(1.0);
    type Panel = (
        &'static str,
        &'static str,
        fn(&DissolutionPoint) -> f64,
        f64,
    );
    let panels: [Panel; 3] = [
        (
            "Transmission (%)",
            "#008b83",
            |p| p.transmission_percent,
            100.0,
        ),
        ("Absorbance (AU)", "#bb642c", |p| p.absorbance_au, 1.0),
        (
            "Estimated dissolved (%)",
            "#447b36",
            |p| p.dissolved_percent,
            100.0,
        ),
    ];
    for (index, (label, color, value, baseline_max)) in panels.into_iter().enumerate() {
        let top = 135.0 + index as f64 * 260.0;
        let bottom = top + 175.0;
        let min = points.iter().map(value).fold(0.0_f64, f64::min);
        let max = points.iter().map(value).fold(baseline_max, f64::max);
        let range = max - min;
        ensure!(range.is_finite() && range > 0.0, "plot range is too large");
        writeln!(
            svg,
            r#"<text x="80" y="{}" font-size="17" font-weight="600">{label}</text>"#,
            top - 15.0
        )?;
        for tick in 0..=4 {
            let fraction = f64::from(tick) / 4.0;
            let y = bottom - fraction * 175.0;
            let level = min + fraction * range;
            writeln!(
                svg,
                r##"<line x1="90" y1="{y}" x2="930" y2="{y}" stroke="#d7ddd4"/><text x="78" y="{}" text-anchor="end" font-size="12">{level:.2}</text>"##,
                y + 4.0
            )?;
            let x = 90.0 + fraction * 840.0;
            let minute = (start + fraction * span) / 60.0;
            writeln!(
                svg,
                r#"<text x="{x}" y="{}" text-anchor="middle" font-size="12">{minute:.2}</text>"#,
                bottom + 22.0
            )?;
        }
        let mut coordinates = String::new();
        for point in points {
            let x = 90.0 + (point.time_s - start) / span * 840.0;
            let y = bottom - (value(point) - min) / range * 175.0;
            write!(coordinates, "{x:.2},{y:.2} ")?;
            writeln!(
                svg,
                r#"<circle cx="{x:.2}" cy="{y:.2}" r="2.5" fill="{color}"/>"#
            )?;
        }
        writeln!(
            svg,
            r#"<polyline points="{coordinates}" fill="none" stroke="{color}" stroke-width="2"/><text x="510" y="{}" text-anchor="middle" font-size="12">Elapsed time (min)</text>"#,
            bottom + 42.0
        )?;
    }
    svg.push_str("<text x=\"80\" y=\"922\" font-size=\"12\">Check quality_flags in dissolution.csv. Values are neither clamped nor smoothed.</text></g></svg>\n");
    Ok(svg)
}
