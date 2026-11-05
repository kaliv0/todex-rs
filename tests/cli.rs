mod common;

use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
};

use common::{APP_OUT, copy_file, copy_sample, run_tdx, tdx, write_good_and_bad};

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
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("cli_out.txt");
    let output = run_tdx(&[
        src.to_str().unwrap(),
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
    let text = fs::read_to_string(&out).unwrap();
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
    let text = fs::read_to_string(&out).unwrap();
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
    let src = copy_file(dir.path(), "app.py").unwrap();
    let output = run_tdx(&[src.to_str().unwrap(), "-s"]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
    assert!(!dir.path().join("TODO").exists());
}

#[test]
fn tee_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("cli_tee_out.txt");
    let output = run_tdx(&[src.to_str().unwrap(), "-o", out.to_str().unwrap(), "--tee"]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&out).unwrap(), APP_OUT);
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

#[test]
fn skips_undecodable_file_and_continues() {
    for debug in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        write_good_and_bad(dir.path()).unwrap();
        let out = dir.path().join("out.txt");
        let path = dir.path().to_str().unwrap();
        let out_s = out.to_str().unwrap();
        let mut cmd = tdx();
        cmd.args([path, "-o", out_s]);
        if debug {
            cmd.arg("-d");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains("TODO: keep me"));
        let err = String::from_utf8_lossy(&output.stderr);
        if debug {
            assert!(err.contains("skipping"), "err={err}");
            assert!(err.contains("bad.bin"), "err={err}");
            assert!(err.to_ascii_lowercase().contains("utf-8"), "err={err}");
        } else {
            assert_eq!(err, "");
        }
    }
}

#[test]
fn skips_undecodable_mid_file_keeps_prior_hits() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mixed.py");
    let mut bytes = b"# TODO: first\n".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', 10000));
    bytes.extend_from_slice(b"\n\xff\xfe bad\n");
    fs::write(&src, bytes).unwrap();
    let out = dir.path().join("out.txt");
    let output = tdx()
        .args([src.to_str().unwrap(), "-o", out.to_str().unwrap(), "-d"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::read_to_string(&out).unwrap().contains("TODO: first"));
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("skipping"), "err={err}");
    assert!(err.contains("mixed.py"), "err={err}");
}

#[test]
fn tee_without_out_uses_todo() {
    let dir = tempfile::tempdir().unwrap();
    copy_file(dir.path(), "app.py").unwrap();
    let output = tdx()
        .current_dir(dir.path())
        .args(["app.py", "--tee"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("TODO")).unwrap(),
        APP_OUT
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
}

#[test]
fn tee_survives_broken_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("out.txt");
    let mut child = tdx()
        .args([src.to_str().unwrap(), "-o", out.to_str().unwrap(), "--tee"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    // tee still finishes the file; broken stdout is not always a hard failure
    assert!(status.success() || status.code() == Some(1));
    assert_eq!(fs::read_to_string(&out).unwrap(), APP_OUT);
}

#[test]
fn stdout_has_no_out_skip() {
    let dir = tempfile::tempdir().unwrap();
    let todo = dir.path().join("TODO");
    fs::write(&todo, "# TODO: keep scanning me\n").unwrap();
    let output = tdx()
        .args([dir.path().to_str().unwrap(), "-s"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("TODO: keep scanning me"));
}
