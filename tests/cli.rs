mod common;

use std::{
    io::Read,
    process::{Command, Output, Stdio},
};

use common::{APP_OUT, copy_sample};

fn tdx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tdx"))
}

fn run_tdx(args: &[&str]) -> Output {
    tdx().args(args).output().expect("run tdx")
}

#[test]
fn missing_dependent_flag_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    for (args, msg) in [
        (
            vec!["-i", path],
            "tdx: error: -i/--ignore-default flag requires -t/--tokens\n",
        ),
        (
            vec!["-m", "20", path],
            "tdx: error: -m/--max-depth flag requires -r/--recursive\n",
        ),
    ] {
        let output = run_tdx(&args);
        assert_eq!(output.status.code(), Some(1), "args={args:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr), msg);
    }
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
        let output = run_tdx(&["-r", "-m", value, path]);
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
    let output = run_tdx(&[
        sample.join("app.py").to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "-t",
        "WARN",
        "-i",
    ]);
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
    let output = run_tdx(&[
        sample.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "-r",
        "-m",
        "1",
    ]);
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
        let mut args = vec![path];
        args.extend(extra.iter().copied());
        let output = run_tdx(&args);
        assert_eq!(output.status.code(), Some(1), "extra={extra:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "tdx: error: shouldn't mix --stdout and --out or --tee flags\n"
        );
    }
}

#[test]
fn s_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let output = run_tdx(&[sample.join("app.py").to_str().unwrap(), "-s"]);
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
    let output = run_tdx(&[
        sample.join("app.py").to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--tee",
    ]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), APP_OUT);
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
}

#[test]
fn missing_path_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_tdx(&[
        dir.path().join("nope").to_str().unwrap(),
        "-o",
        dir.path().join("x").to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.starts_with("tdx: error:") && err.contains("not found"),
        "err={err}"
    );
}

#[test]
fn s_broken_pipe_exits_zero() {
    for extra in [vec!["-s"], vec!["-s", "-d"]] {
        let dir = tempfile::tempdir().unwrap();
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
        }
        let output = child.wait_with_output().expect("wait tdx");
        // silent like Python; exit 0 rg-style (Python used 141)
        assert_eq!(output.status.code(), Some(0), "args={args:?}");
        assert!(
            output.stderr.is_empty(),
            "stderr should be quiet on broken pipe, got={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn keyboard_interrupt_exits_130() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    for i in 0..3000 {
        std::fs::write(
            dir.path().join(format!("f{i}.py")),
            format!("# TODO: item-{i}\n"),
        )
        .unwrap();
    }
    let child = tdx()
        .args([dir.path().to_str().unwrap(), "-o", out.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tdx");
    let pid = child.id();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let kill = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("kill -INT");
    assert!(kill.success(), "kill -INT failed");
    let output = child.wait_with_output().expect("wait tdx");
    assert_eq!(
        output.status.code(),
        Some(130),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr should be quiet on interrupt, got={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
