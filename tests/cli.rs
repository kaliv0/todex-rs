mod common;

use std::io::Read;
use std::process::{Command, Stdio};

use common::{APP_OUT, copy_sample};

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
    assert_eq!(output.status.code(), Some(1));
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
    assert_eq!(output.status.code(), Some(1));
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

#[test]
fn stdout_mix_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    for extra in [vec!["-s", "-o", "x"], vec!["-s", "--tee"]] {
        let output = tdx().args([path]).args(&extra).output().expect("run tdx");
        assert_eq!(output.status.code(), Some(1), "extra={extra:?}");
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            err,
            "tdx: error: shouldn't mix --stdout and --out or --tee flags\n"
        );
    }
}

#[test]
fn s_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let output = tdx()
        .args([sample.join("app.py").to_str().unwrap(), "-s"])
        .output()
        .expect("run tdx");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
    assert!(!sample.join("TODO").exists());
}

#[test]
fn tee_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("cli_tee_out.txt");
    let output = tdx()
        .args([
            sample.join("app.py").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--tee",
        ])
        .output()
        .expect("run tdx");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), APP_OUT);
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
}

#[test]
fn s_broken_pipe_exits_nonzero() {
    for extra in [vec!["-s"], vec!["-s", "-d"]] {
        let dir = tempfile::tempdir().unwrap();
        // many hits so writes continue after the reader closes (small out fits in the pipe buffer)
        for i in 0..400 {
            std::fs::write(
                dir.path().join(format!("f{i}.py")),
                format!("# TODO: item-{i}\n"),
            )
            .unwrap();
        }
        let mut args = vec![dir.path().to_str().unwrap().to_string()];
        args.extend(extra.into_iter().map(str::to_string));
        let mut child = tdx()
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn tdx");
        {
            let mut stdout = child.stdout.take().expect("stdout");
            let mut buf = [0u8; 1];
            let _ = stdout.read(&mut buf);
            // drop stdout -> broken pipe for later writes
        }
        let output = child.wait_with_output().expect("wait tdx");
        // silent like Python; exit 1 (Python used 141)
        assert_eq!(output.status.code(), Some(1), "args={args:?}");
        assert!(
            output.stderr.is_empty(),
            "stderr should be quiet on broken pipe, got={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
