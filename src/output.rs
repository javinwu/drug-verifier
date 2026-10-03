use crate::{config::ExperimentConfig, pill_sensor::DissolutionPoint};
use anyhow::{Result, ensure};
use std::{fmt::Write as _, fs, path::Path};

/// Export raw and derived data together, calibration, and a standalone SVG plot.
pub fn write_report(
    directory: &Path,
    points: &[DissolutionPoint],
    config: &ExperimentConfig,
    simulated: bool,
) -> Result<()> {
    ensure!(!points.is_empty(), "cannot export an empty experiment");
    // Build the plot first so invalid data never produces a partial CSV report.
    let svg = render_svg(points, simulated)?;
    fs::create_dir_all(directory)?;
    let mut writer = csv::Writer::from_path(directory.join("dissolution.csv"))?;
    for point in points {
        writer.serialize(point)?;
    }
    writer.flush()?;
    fs::write(directory.join("dissolution.svg"), svg)?;
    fs::write(
        directory.join("calibration.toml"),
        toml::to_string_pretty(config)?,
    )?;
    let source = if simulated {
        "SIMULATED DATA"
    } else {
        "CSV INPUT"
    };
    fs::write(
        directory.join("run.txt"),
        format!(
            "Peel {} | {source}\n{} readings\nDissolution estimates use calibration.toml.\nConstant vessel volume; no withdrawn samples or dilution correction.\n",
            env!("CARGO_PKG_VERSION"),
            points.len()
        ),
    )?;
    Ok(())
}

/// Raw points are joined by straight lines; no smoothing or kinetic fit is applied.
pub fn render_svg(points: &[DissolutionPoint], simulated: bool) -> Result<String> {
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
    let source = if simulated {
        "SIMULATED DATA"
    } else {
        "CSV SENSOR INPUT"
    };
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
