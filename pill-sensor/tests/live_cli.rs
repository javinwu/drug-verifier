use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct ServerProcess {
    child: Child,
    address: String,
    out: PathBuf,
}
impl ServerProcess {
    fn start(extra: &[&str]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let out = std::env::temp_dir().join(format!(
            "peel-http-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let child = Command::new(env!("CARGO_BIN_EXE_peel"))
            .args([
                "live",
                "--port",
                "0",
                "--config",
                "config/example.toml",
                "--out",
            ])
            .arg(&out)
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut server = Self {
            child,
            address: String::new(),
            out,
        };
        let stdout = server.child.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stdout).read_line(&mut line).unwrap();
            let _ = send.send(line);
        });
        let line = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("server startup timeout");
        server.address = line
            .trim()
            .strip_prefix("Peel live: http://")
            .expect("server address")
            .to_owned();
        server
    }

    fn get(&self, path: &str) -> String {
        let mut socket = TcpStream::connect(&self.address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            socket,
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        )
        .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.to_lowercase().contains("cache-control: no-store"));
        response.split_once("\r\n\r\n").unwrap().1.to_owned()
    }

    fn status(&self) -> Value {
        serde_json::from_str(&self.get("/api/status")).unwrap()
    }

    fn until(&self, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = self.status();
            if predicate(&status) {
                return status;
            }
            assert!(Instant::now() < deadline, "timeout: {status}");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.out);
    }
}

const PACKET: &str =
    "{\"time_s\":0,\"reference_intensity\":4010,\"sample_intensity\":4010,\"dark_intensity\":10}";

#[test]
fn live_http_updates_before_stdin_eof_and_preserves_partial_packets() {
    let mut server = ServerProcess::start(&["--input", "-", "--synthetic-input"]);
    assert!(server.get("/").contains("Live dissolution"));
    assert!(server.get("/app.js").contains("/api/status"));
    server
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(PACKET.as_bytes())
        .unwrap();
    assert_eq!(server.status()["count"], 0);
    server
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let status = server.until(|s| s["count"] == 1);
    assert_eq!(status["status"], "running");
    assert_eq!(status["source"], "SIMULATED DATA");
    assert!(server.get("/curve.svg").contains("SIMULATED DATA"));
    let next = PACKET
        .replace("\"time_s\":0", "\"time_s\":60")
        .replace("\"sample_intensity\":4010", "\"sample_intensity\":410");
    writeln!(server.child.stdin.as_mut().unwrap(), "{next}").unwrap();
    server.until(|s| s["count"] == 2);
    drop(server.child.stdin.take());
    let complete = server.until(|s| s["status"] == "complete");
    assert_eq!(complete["latest"]["dissolved_percent"], 100.0);
    assert_eq!(server.get("/dissolution.csv").lines().count(), 3);
    assert_eq!(server.get("/raw.ndjson").lines().count(), 2);
}

#[test]
fn stream_failure_is_visible_over_http_and_retains_the_valid_curve() {
    let mut server = ServerProcess::start(&["--input", "-"]);
    writeln!(
        server.child.stdin.as_mut().unwrap(),
        "{PACKET}\nbroken packet"
    )
    .unwrap();
    let status = server.until(|s| s["status"] == "error");
    assert_eq!(status["count"], 1);
    assert!(status["error"].as_str().unwrap().contains("line 2"));
    assert!(server.get("/curve.svg").contains("EXTERNAL SENSOR STREAM"));
    assert!(server.get("/run.txt").contains("Live status: error"));
}

#[test]
fn fake_device_stdout_can_feed_the_live_receiver() {
    let mut server = ServerProcess::start(&["--input", "-", "--synthetic-input"]);
    let device = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args(["emit", "--samples", "3", "--speed", "1000000"])
        .stdout(Stdio::from(server.child.stdin.take().unwrap()))
        .output()
        .unwrap();
    assert!(device.status.success());
    let status = server.until(|s| s["status"] == "complete");
    assert_eq!(status["count"], 3);
    assert_eq!(status["latest"]["time_s"], 60.0);
}

#[test]
fn recorded_input_requires_calibration_and_cannot_overwrite_itself() {
    let failure = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args(["live", "--input", "-"])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("explicit --config"));
    let server = ServerProcess::start(&["--samples", "2", "--speed", "1000000"]);
    server.until(|s| s["status"] == "complete");
    let input = server.out.join("raw.ndjson");
    let before = fs::read(&input).unwrap();
    let failure = Command::new(env!("CARGO_BIN_EXE_peel"))
        .args(["live", "--config", "config/example.toml", "--input"])
        .arg(&input)
        .arg("--out")
        .arg(&server.out)
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("would be overwritten"));
    assert_eq!(fs::read(input).unwrap(), before);
}
