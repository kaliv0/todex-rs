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
