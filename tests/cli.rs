use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "peel-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn demo_exports_data_calibration_and_three_panel_curve() {
    let dir = TempDirectory::new();
    let result = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args(["simulate", "--out"])
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("SIMULATED DATA"));
    let mut csv = csv::Reader::from_path(dir.0.join("dissolution.csv")).unwrap();
    assert!(csv.headers().unwrap().iter().any(|h| h == "quality_flags"));
    assert_eq!(csv.records().count(), 121);
    let svg = fs::read_to_string(dir.0.join("dissolution.svg")).unwrap();
    assert_eq!(svg.matches("<polyline ").count(), 3);
    assert!(svg.contains("SIMULATED DATA"));
    assert!(dir.0.join("calibration.toml").exists());
    assert!(dir.0.join("run.txt").exists());
}

#[test]
fn analyze_example_csv_and_fail_cleanly_on_invalid_input() {
    let dir = TempDirectory::new();
    let output = dir.0.join("valid");
    let result = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args([
            "analyze",
            "--input",
            "data/example_readings.csv",
            "--config",
            "config/example.toml",
            "--out",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("100.00% estimated dissolved"));
    let bad_input = dir.0.join("bad.csv");
    fs::write(
        &bad_input,
        "time_s,reference_intensity,sample_intensity,dark_intensity\n0,4010,10,10\n",
    )
    .unwrap();
    let invalid_output = dir.0.join("invalid");
    let failure = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args(["analyze", "--input"])
        .arg(bad_input)
        .args(["--config", "config/example.toml", "--out"])
        .arg(&invalid_output)
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("sample must exceed dark"));
    assert!(!invalid_output.exists());
}

#[test]
fn analyze_accepts_stdin() {
    let dir = TempDirectory::new();
    let result = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args([
            "analyze",
            "--input",
            "-",
            "--config",
            "config/example.toml",
            "--out",
        ])
        .arg(&dir.0)
        .stdin(fs::File::open("data/example_readings.csv").unwrap())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(dir.0.join("dissolution.csv").exists());
}
