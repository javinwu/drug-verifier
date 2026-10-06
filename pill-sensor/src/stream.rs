//! The device boundary: one newline-delimited JSON object per measurement.
use crate::{pill_sensor::RawReading, source::ReadingSource};
use anyhow::{Context, Result, ensure};
use std::{
    io::{BufRead, Cursor, Read, Write},
    thread,
    time::Duration,
};

const MAX_PACKET_BYTES: u64 = 4096;

pub fn write_packet(writer: &mut impl Write, reading: &RawReading) -> Result<()> {
    let mut packet = serde_json::to_vec(reading)?;
    packet.push(b'\n');
    writer.write_all(&packet)?;
    writer.flush()?;
    Ok(())
}

pub struct JsonLinesSource<R> {
    reader: R,
    line: usize,
}

impl<R: BufRead> JsonLinesSource<R> {
    pub fn new(reader: R) -> Self {
        Self { reader, line: 0 }
    }
}

impl<R: BufRead> ReadingSource for JsonLinesSource<R> {
    fn next_reading(&mut self) -> Result<Option<RawReading>> {
        let mut packet = String::new();
        let count = self
            .reader
            .by_ref()
            .take(MAX_PACKET_BYTES + 1)
            .read_line(&mut packet)?;
        if count == 0 {
            return Ok(None);
        }
        self.line += 1;
        ensure!(
            count as u64 <= MAX_PACKET_BYTES,
            "packet {} exceeds 4096 bytes",
            self.line
        );
        ensure!(
            packet.ends_with('\n'),
            "incomplete packet {}: expected newline before EOF",
            self.line
        );
        serde_json::from_str(&packet)
            .with_context(|| format!("invalid JSON sensor packet on line {}", self.line))
            .map(Some)
    }
}

/// Exercise exactly the wire encoder and parser even for the one-command demo.
pub struct PacketRoundTrip<S>(pub S);

impl<S: ReadingSource> ReadingSource for PacketRoundTrip<S> {
    fn next_reading(&mut self) -> Result<Option<RawReading>> {
        let Some(reading) = self.0.next_reading()? else {
            return Ok(None);
        };
        let mut packet = Vec::new();
        write_packet(&mut packet, &reading)?;
        JsonLinesSource::new(Cursor::new(packet)).next_reading()
    }
}

/// Pace by experiment timestamps; speed changes waiting, never measurement time.
pub struct PacedSource<S> {
    source: S,
    speed: f64,
    previous_time: Option<f64>,
}

impl<S> PacedSource<S> {
    pub fn new(source: S, speed: f64) -> Result<Self> {
        ensure!(
            speed.is_finite() && speed > 0.0,
            "speed must be finite and positive"
        );
        Ok(Self {
            source,
            speed,
            previous_time: None,
        })
    }
}

impl<S: ReadingSource> ReadingSource for PacedSource<S> {
    fn next_reading(&mut self) -> Result<Option<RawReading>> {
        let Some(reading) = self.source.next_reading()? else {
            return Ok(None);
        };
        ensure!(
            reading.time_s.is_finite() && reading.time_s >= 0.0,
            "invalid timestamp"
        );
        if let Some(previous) = self.previous_time {
            ensure!(
                reading.time_s > previous,
                "timestamps must strictly increase"
            );
            let delay = Duration::try_from_secs_f64((reading.time_s - previous) / self.speed)
                .context("timestamp interval cannot be paced at this speed")?;
            thread::sleep(delay);
        }
        self.previous_time = Some(reading.time_s);
        Ok(Some(reading))
    }
}
