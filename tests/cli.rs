mod common;

use std::process::Command;

use common::copy_sample;

fn tdx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tdx"))
}

#[test]
fn missing_dependent_flag_errors_for_i() {
    let dir = tempfile::tempdir().unwrap();
    let output = tdx()
        .args(["-i", dir.path().to_str().unwrap()])
        .output()
        .expect("run tdx");
    assert_eq!(output.status.code(), Some(2));
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        err,
        "tdx: error: -i/--ignore-default flag requires -t/--tokens\n"
    );
}

#[test]
fn missing_dependent_flag_errors_for_m() {
    let dir = tempfile::tempdir().unwrap();
    let output = tdx()
        .args(["-m", "20", dir.path().to_str().unwrap()])
        .output()
        .expect("run tdx");
    assert_eq!(output.status.code(), Some(2));
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        err,
        "tdx: error: -m/--max-depth flag requires -r/--recursive\n"
    );
}

#[test]
fn max_depth_rejects_invalid_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    for (value, needle) in [
        ("0", "must be positive"),
        ("-1", "must be positive"),
        ("xyz", "is not a valid number"),
    ] {
        let output = tdx()
            .args(["-r", "-m", value, path])
            .output()
            .expect("run tdx");
        assert_eq!(output.status.code(), Some(2), "value={value}");
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(
            err.contains(needle),
            "value={value} needle={needle} err={err}"
        );
    }
}

#[test]
fn argv_reaches_extractor() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("cli_out.txt");
    let output = tdx()
        .args([
            sample.join("app.py").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "-t",
            "WARN",
            "-i",
        ])
        .output()
        .expect("run tdx");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("WARN"));
    assert!(!text.contains("TODO: main"));
}

#[test]
fn r_m_limits_depth_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("cli_depth_out.txt");
    let output = tdx()
        .args([
            sample.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "-r",
            "-m",
            "1",
        ])
        .output()
        .expect("run tdx");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("TODO: deep"));
    assert!(!text.contains("TODO: deeper"));
}
