use peel::{
    analyze,
    config::ExperimentConfig,
    output::render_svg,
    pill_sensor::{PillSensor, RawReading},
    source::{CsvSource, SimulatedSource},
};

fn config() -> ExperimentConfig {
    ExperimentConfig {
        slope_au_per_mg_ml: 1.8,
        intercept_au: 0.0,
        vessel_volume_ml: 900.0,
        pill_mass_mg: 500.0,
    }
}

fn reading(time_s: f64, sample: f64) -> RawReading {
    RawReading {
        time_s,
        reference_intensity: 4010.0,
        sample_intensity: sample,
        dark_intensity: 10.0,
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn known_ten_percent_transmission_gives_one_au_and_full_dose() {
    let point = PillSensor::new(config())
        .unwrap()
        .process(reading(0.0, 410.0))
        .unwrap();
    close(point.transmittance, 0.1);
    close(point.transmission_percent, 10.0);
    close(point.absorbance_au, 1.0);
    close(point.concentration_mg_ml, 500.0 / 900.0);
    close(point.dissolved_mass_mg, 500.0);
    close(point.dissolved_percent, 100.0);
}

#[test]
fn blank_is_zero_dissolution() {
    let point = PillSensor::new(config())
        .unwrap()
        .process(reading(0.0, 4010.0))
        .unwrap();
    close(point.transmittance, 1.0);
    close(point.absorbance_au, 0.0);
    close(point.dissolved_percent, 0.0);
    assert!(point.quality_flags.is_empty());
}

#[test]
fn calibration_intercept_is_subtracted() {
    let mut calibration = config();
    calibration.intercept_au = 0.2;
    let point = PillSensor::new(calibration)
        .unwrap()
        .process(reading(0.0, 410.0))
        .unwrap();
    close(point.dissolved_percent, 80.0);
}

#[test]
fn implausible_values_are_flagged_and_preserved() {
    let mut sensor = PillSensor::new(config()).unwrap();
    let negative = sensor.process(reading(0.0, 8010.0)).unwrap();
    assert!(negative.dissolved_percent < 0.0);
    assert!(
        negative
            .quality_flags
            .contains("transmission_above_100_percent")
    );
    assert!(negative.quality_flags.contains("negative_concentration"));
    let high = sensor.process(reading(1.0, 50.0)).unwrap();
    close(high.dissolved_percent, 200.0);
    assert!(high.quality_flags.contains("dissolution_above_100_percent"));
}

#[test]
fn invalid_signals_and_nonfinite_inputs_fail() {
    let base = reading(0.0, 100.0);
    let invalid = [
        RawReading {
            sample_intensity: 10.0,
            ..base
        },
        RawReading {
            sample_intensity: 0.0,
            ..base
        },
        RawReading {
            reference_intensity: 10.0,
            ..base
        },
        RawReading {
            reference_intensity: f64::NAN,
            ..base
        },
        RawReading {
            sample_intensity: f64::INFINITY,
            ..base
        },
        RawReading {
            dark_intensity: -1.0,
            ..base
        },
        RawReading {
            time_s: -1.0,
            ..base
        },
    ];
    for reading in invalid {
        assert!(PillSensor::new(config()).unwrap().process(reading).is_err());
    }
}

#[test]
fn invalid_calibration_is_rejected() {
    for invalid in [
        ExperimentConfig {
            slope_au_per_mg_ml: 0.0,
            ..config()
        },
        ExperimentConfig {
            slope_au_per_mg_ml: -1.0,
            ..config()
        },
        ExperimentConfig {
            vessel_volume_ml: f64::INFINITY,
            ..config()
        },
        ExperimentConfig {
            pill_mass_mg: 0.0,
            ..config()
        },
        ExperimentConfig {
            intercept_au: f64::NAN,
            ..config()
        },
    ] {
        assert!(PillSensor::new(invalid).is_err());
    }
}

#[test]
fn overflow_is_an_error() {
    let calibration = ExperimentConfig {
        slope_au_per_mg_ml: f64::MIN_POSITIVE,
        ..config()
    };
    assert!(
        PillSensor::new(calibration)
            .unwrap()
            .process(reading(0.0, 410.0))
            .is_err()
    );
}

#[test]
fn times_must_increase_and_failed_readings_do_not_advance_time() {
    let mut sensor = PillSensor::new(config()).unwrap();
    sensor.process(reading(10.0, 4010.0)).unwrap();
    assert!(sensor.process(reading(10.0, 4000.0)).is_err());
    assert!(sensor.process(reading(9.0, 4000.0)).is_err());
    assert!(sensor.process(reading(20.0, 0.0)).is_err());
    sensor.process(reading(15.0, 4000.0)).unwrap();
}

#[test]
fn csv_handles_reordered_headers_and_rejects_bad_rows() {
    let csv = "sample_intensity,time_s,dark_intensity,reference_intensity\n410,0,10,4010\n";
    let points = analyze(&mut CsvSource::new(csv.as_bytes()).unwrap(), config()).unwrap();
    close(points[0].dissolved_percent, 100.0);
    let bad = "time_s,reference_intensity,sample_intensity,dark_intensity\n0,4010,bad,10\n";
    assert!(analyze(&mut CsvSource::new(bad.as_bytes()).unwrap(), config()).is_err());
}

#[test]
fn missing_duplicate_and_empty_csv_data_fail() {
    assert!(CsvSource::new("time_s,sample_intensity\n".as_bytes()).is_err());
    assert!(CsvSource::new("time_s,time_s,sample_intensity,dark_intensity\n".as_bytes()).is_err());
    let header = "time_s,reference_intensity,sample_intensity,dark_intensity\n";
    assert!(analyze(&mut CsvSource::new(header.as_bytes()).unwrap(), config()).is_err());
}

#[test]
fn simulated_curve_matches_analytic_first_order_model() {
    let mut source = SimulatedSource::new(config(), 3, 600.0, 600.0).unwrap();
    let points = analyze(&mut source, config()).unwrap();
    close(points[0].dissolved_percent, 0.0);
    close(
        points[1].dissolved_percent,
        (1.0 - (-1.0_f64).exp()) * 100.0,
    );
    close(
        points[2].dissolved_percent,
        (1.0 - (-2.0_f64).exp()) * 100.0,
    );
    assert!(points.iter().all(|p| p.quality_flags.is_empty()));
}

#[test]
fn invalid_simulation_arguments_fail() {
    assert!(SimulatedSource::new(config(), 0, 30.0, 600.0).is_err());
    assert!(SimulatedSource::new(config(), 2, 0.0, 600.0).is_err());
    assert!(SimulatedSource::new(config(), 2, 30.0, f64::NAN).is_err());
}

#[test]
fn plot_supports_a_single_reading_and_rejects_empty_data() {
    let point = PillSensor::new(config())
        .unwrap()
        .process(reading(0.0, 4010.0))
        .unwrap();
    let svg = render_svg(&[point], false).unwrap();
    assert!(svg.contains("CSV SENSOR INPUT"));
    assert!(!svg.contains("NaN"));
    assert!(!svg.contains("inf"));
    assert!(render_svg(&[], false).is_err());
}
