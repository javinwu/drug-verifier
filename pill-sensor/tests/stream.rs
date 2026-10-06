use peel::{
    analyze,
    config::ExperimentConfig,
    live::collect_live,
    source::{ReadingSource, SimulatedSource},
    stream::{JsonLinesSource, PacedSource, PacketRoundTrip, write_packet},
};
use std::{
    fs,
    io::{BufReader, Cursor},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn config() -> ExperimentConfig {
    ExperimentConfig::load(std::path::Path::new("config/example.toml")).unwrap()
}

fn simulator() -> SimulatedSource {
    SimulatedSource::new(config(), 5, 60.0, 600.0).unwrap()
}

const FIRST: &str =
    "{\"time_s\":0,\"reference_intensity\":4010,\"sample_intensity\":4010,\"dark_intensity\":10}\n";

struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "peel-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn wire_round_trip_matches_existing_batch_math() {
    let expected = analyze(&mut simulator(), config()).unwrap();
    let actual = analyze(&mut PacketRoundTrip(simulator()), config()).unwrap();
    assert_eq!(
        serde_json::to_value(expected).unwrap(),
        serde_json::to_value(actual).unwrap()
    );
}

#[test]
fn packets_survive_fragmented_reads_and_crlf() {
    let bytes = FIRST.replace('\n', "\r\n") + FIRST;
    let mut source = JsonLinesSource::new(BufReader::with_capacity(1, Cursor::new(bytes)));
    assert_eq!(
        source.next_reading().unwrap().unwrap().sample_intensity,
        4010.0
    );
    assert_eq!(source.next_reading().unwrap().unwrap().time_s, 0.0);
    assert!(source.next_reading().unwrap().is_none());
}

#[test]
fn malformed_incomplete_and_oversized_packets_fail() {
    for packet in [
        "\n".to_owned(),
        "{}\n".to_owned(),
        "not json\n".to_owned(),
        FIRST.trim_end().to_owned(),
        FIRST.replace("4010", "NaN"),
        FIRST.replace("\"time_s\":0", "\"time_s\":1e999"),
        FIRST.replace("\"time_s\":0", "\"time_s\":0,\"time_s\":1"),
        FIRST.replace("\"time_s\":0", "\"time_s\":0,\"unexpected\":1"),
        format!("{}\n", " ".repeat(4097)),
    ] {
        assert!(
            JsonLinesSource::new(Cursor::new(&packet))
                .next_reading()
                .is_err(),
            "accepted {packet}"
        );
    }
}

#[test]
fn pacing_validates_speed_and_preserves_experiment_timestamps() {
    for speed in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(PacedSource::new(simulator(), speed).is_err());
    }
    let mut paced = PacedSource::new(simulator(), 1e9).unwrap();
    assert_eq!(paced.next_reading().unwrap().unwrap().time_s, 0.0);
    assert_eq!(paced.next_reading().unwrap().unwrap().time_s, 60.0);
    let mut reversed = PacedSource::new(
        JsonLinesSource::new(Cursor::new(format!("{FIRST}{FIRST}"))),
        60.0,
    )
    .unwrap();
    reversed.next_reading().unwrap();
    assert!(reversed.next_reading().is_err());
}

#[test]
fn live_publishes_and_saves_every_point_then_completes() {
    let dir = TempDirectory::new();
    let mut statuses = Vec::new();
    collect_live(
        &mut PacketRoundTrip(simulator()),
        config(),
        &dir.0,
        true,
        |snapshot| {
            let csv = fs::read_to_string(dir.0.join("dissolution.csv")).unwrap();
            assert_eq!(csv.lines().count(), snapshot.count + 1);
            assert_eq!(
                fs::read_to_string(dir.0.join("dissolution.svg")).unwrap(),
                snapshot.svg.unwrap()
            );
            statuses.push((snapshot.status, snapshot.count));
        },
    )
    .unwrap();
    assert_eq!(
        statuses,
        vec![
            ("running", 1),
            ("running", 2),
            ("running", 3),
            ("running", 4),
            ("running", 5),
            ("complete", 5)
        ]
    );
    let recorded = fs::read_to_string(dir.0.join("raw.ndjson")).unwrap();
    let replay = analyze(&mut JsonLinesSource::new(Cursor::new(recorded)), config()).unwrap();
    let expected = analyze(&mut simulator(), config()).unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert!(
        fs::read_to_string(dir.0.join("run.txt"))
            .unwrap()
            .contains("Live status: complete")
    );
}

#[test]
fn live_failure_retains_valid_history_and_records_failure() {
    for invalid in [
        "bad json\n".to_owned(),
        FIRST.to_owned(),
        FIRST
            .replace("\"time_s\":0", "\"time_s\":60")
            .replace("\"sample_intensity\":4010", "\"sample_intensity\":10"),
    ] {
        let dir = TempDirectory::new();
        let mut last = None;
        let mut source = JsonLinesSource::new(Cursor::new(format!("{FIRST}{invalid}")));
        assert!(collect_live(&mut source, config(), &dir.0, false, |s| last = Some(s)).is_err());
        let last = last.unwrap();
        assert_eq!(last.status, "error");
        assert_eq!(last.count, 1);
        assert!(last.error.is_some());
        assert!(last.svg.unwrap().contains("EXTERNAL SENSOR STREAM"));
        assert_eq!(
            fs::read_to_string(dir.0.join("dissolution.csv"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert!(
            fs::read_to_string(dir.0.join("run.txt"))
                .unwrap()
                .contains("Live status: error")
        );
    }
}

#[test]
fn empty_stream_clears_previous_graph_and_is_an_error() {
    let dir = TempDirectory::new();
    fs::write(dir.0.join("dissolution.svg"), "old graph").unwrap();
    let mut last = None;
    assert!(
        collect_live(
            &mut JsonLinesSource::new(Cursor::new("")),
            config(),
            &dir.0,
            true,
            |s| last = Some(s)
        )
        .is_err()
    );
    assert_eq!(last.unwrap().count, 0);
    assert!(fs::read(dir.0.join("dissolution.svg")).unwrap().is_empty());
}

#[test]
fn encoder_emits_exactly_one_newline_delimited_packet() {
    let reading = simulator().next_reading().unwrap().unwrap();
    let mut bytes = Vec::new();
    write_packet(&mut bytes, &reading).unwrap();
    assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1);
    assert!(bytes.ends_with(b"\n"));
    assert!(
        JsonLinesSource::new(Cursor::new(bytes))
            .next_reading()
            .unwrap()
            .is_some()
    );
}
