mod common;

use std::fs;

use common::{
    APP_OUT, copy_sample, no_interrupt, run_extract, run_extract_full, run_extract_with,
    write_good_and_bad,
};
use todex::extractor::Extractor;

#[test]
fn default_scan() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract(&sample.join("app.py"), &out).unwrap();
    assert!(text.contains("TODO: main"));
    assert!(text.contains("FIXME: bug"));
    assert!(!text.contains("WARN: careful"));
}

#[test]
fn t_adds_token_and_keeps_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(
        &sample.join("app.py"),
        &out,
        Some(vec!["WARN".into()]),
        false,
        false,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("WARN: careful"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn i_with_t_uses_only_custom_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(
        &sample.join("app.py"),
        &out,
        Some(vec!["WARN".into()]),
        true,
        false,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("WARN: careful"));
    assert!(!text.contains("TODO: main"));
}

#[test]
fn custom_tokens_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(
        &sample.join("custom_tokens.py"),
        &out,
        Some(vec![
            "NOTE".into(),
            "!!!URGENT!!!".into(),
            "@FIXME".into(),
            "中文标记".into(),
        ]),
        true,
        false,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("plain note"));
    assert!(text.contains("!!!URGENT!!!"));
    assert!(text.contains("at-prefixed fixme"));
    assert!(text.contains("non-ascii"));
}

#[test]
fn o_dir_writes_todo_inside() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let reports = sample.join("reports");
    fs::create_dir(&reports).unwrap();

    let mut ext = Extractor::new(
        sample.join("app.py"),
        None,
        false,
        None,
        false,
        false,
        Some(reports.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    )
    .unwrap();
    ext.run().unwrap();

    assert_eq!(fs::read_to_string(reports.join("TODO")).unwrap(), APP_OUT);
}

#[test]
fn omitted_out_defaults_to_todo() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&sample).unwrap();

    let result = (|| -> todex::error::Result<_> {
        let mut ext = Extractor::new(
            "app.py",
            None,
            false,
            None,
            false,
            false,
            None,
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            no_interrupt(),
        )?;
        ext.run()?;
        Ok(fs::read_to_string("TODO")?)
    })();

    std::env::set_current_dir(prev).unwrap();
    assert_eq!(result.unwrap(), APP_OUT);
}

#[test]
fn zero_token_file_empty_out() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in [("empty.py", ""), ("nl.py", "\n")] {
        let src = dir.path().join(name);
        let out = dir.path().join(format!("{name}.out"));
        fs::write(&src, content).unwrap();
        let mut ext = Extractor::new(
            &src,
            None,
            false,
            None,
            false,
            false,
            Some(out.to_str().unwrap()),
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            no_interrupt(),
        )
        .unwrap();
        ext.run().unwrap();
        assert_eq!(fs::read_to_string(&out).unwrap(), "");
    }
}

#[test]
fn o_equals_scanned_file_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.py");
    fs::write(&src, "# TODO: will be truncated\n").unwrap();

    let mut ext = Extractor::new(
        &src,
        None,
        false,
        None,
        false,
        false,
        Some(src.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    )
    .unwrap();
    ext.run().unwrap();

    assert_eq!(fs::read_to_string(&src).unwrap(), "");
}

#[test]
fn zero_hit_scan_truncates_stale_out() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("empty.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "print(1)\n").unwrap();
    fs::write(&out, "stale from previous run\n").unwrap();

    let mut ext = Extractor::new(
        &src,
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    )
    .unwrap();
    ext.run().unwrap();

    assert_eq!(fs::read_to_string(&out).unwrap(), "");
}

#[test]
fn missing_path_raises() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let out = dir.path().join("out.txt");
    let err = match Extractor::new(
        &missing,
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    ) {
        Err(e) => e,
        Ok(_) => panic!("expected missing path to fail in new"),
    };
    assert!(err.to_string().contains("not found"));
    assert!(!out.exists());
}

// --- concise / snippets ---

#[test]
fn c_omits_snippet_body() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(
        &sample.join("snippet.py"),
        &out,
        None,
        false,
        true,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("FIXME"));
    assert!(!text.contains("return 1"));
}

#[test]
fn snippet_includes_following_lines() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract(&sample.join("snippet.py"), &out).unwrap();
    assert!(text.contains("return 1"));
}

#[test]
fn warn_snippet_count() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("warn.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "# WARN{2} check this\nx = 1\ny = 2\n").unwrap();
    let text = run_extract_with(
        &src,
        &out,
        Some(vec!["WARN".into()]),
        true,
        false,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("WARN"));
    assert!(text.contains("x = 1"));
    assert!(!text.contains("y = 2"));
}

#[test]
fn snippet_past_eof_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("short.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "# TODO{10} almost eof\nonly one more\n").unwrap();
    let text = run_extract(&src, &out).unwrap();
    assert!(text.contains("TODO"));
    assert!(text.contains("only one more"));
}

#[test]
fn invalid_snippet_raises() {
    let dir = tempfile::tempdir().unwrap();
    for content in ["# TODO{3 no close\n", "# TODO{abc}\n", "# TODO{-1}\n"] {
        for concise in [false, true] {
            let src = dir.path().join("bad.py");
            let out = dir.path().join("out.txt");
            fs::write(&src, content).unwrap();
            let mut ext = Extractor::new(
                &src,
                None,
                false,
                None,
                false,
                false,
                Some(out.to_str().unwrap()),
                false,
                concise,
                false,
                None,
                false,
                false,
                false,
                no_interrupt(),
            )
            .unwrap();
            let err = ext.run().unwrap_err();
            assert!(
                err.to_string().contains("invalid snippet count"),
                "err={err}"
            );
        }
    }
}

#[test]
fn multiple_hits_one_header_and_line_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("multi.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "line1\n# TODO{2} first\nbody\n# FIXME: second\n").unwrap();
    let text = run_extract(&src, &out).unwrap();
    assert_eq!(text.matches("======================").count(), 1);
    assert!(text.contains("line 2:"));
    assert!(text.contains("line 4:"));
    assert!(text.contains("TODO"));
    assert!(text.contains("FIXME"));
    assert!(text.contains("body"));
}

#[test]
fn f_writes_absolute_path() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let target = sample.join("app.py");
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &target, &out, None, false, None, false, false, true, false, false, None,
    )
    .unwrap();
    let abs = std::path::absolute(&target).unwrap();
    assert!(
        text.contains(&abs.to_string_lossy().to_string()),
        "expected absolute path in header, got:\n{text}"
    );
}

#[test]
fn without_r_skips_nested() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract(&sample, &out).unwrap();
    assert!(!text.contains("TODO: deep"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn dir_walk_order_depth0_files_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in ["c.py", "a.py", "b.py"] {
        fs::write(root.join(name), format!("# TODO: {name}\n")).unwrap();
    }
    let nested = root.join("d");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("z.py"), "# TODO: nested\n").unwrap();
    let out = root.join("out.txt");
    let text = run_extract(root, &out).unwrap();
    assert!(text.find("TODO: a.py").unwrap() < text.find("TODO: b.py").unwrap());
    assert!(text.find("TODO: b.py").unwrap() < text.find("TODO: c.py").unwrap());
    assert!(!text.contains("TODO: nested"));
}

#[test]
fn symlink_not_followed_file() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let target = outside.join("target.py");
    fs::write(&target, "# TODO: via-file-link\n").unwrap();

    let scan = dir.path().join("scan");
    fs::create_dir(&scan).unwrap();
    fs::write(scan.join("top.py"), "# TODO: top\n").unwrap();
    std::os::unix::fs::symlink(&target, scan.join("link.py")).unwrap();

    let out = scan.join("out.txt");
    let text = run_extract(&scan, &out).unwrap();
    assert!(text.contains("TODO: top"));
    assert!(!text.contains("TODO: via-file-link"));
}

#[test]
fn symlink_path_raises_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file.py");
    fs::write(&target, "# TODO: x\n").unwrap();
    let link = dir.path().join("link.py");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let out = dir.path().join("out.txt");
    let err = match Extractor::new(
        &link,
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    ) {
        Err(e) => e,
        Ok(_) => panic!("expected symlink path to fail in new"),
    };
    assert!(err.to_string().contains("is a symlink"));
}

#[test]
fn symlink_path_raises_dir() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("dir");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("x.py"), "# TODO: x\n").unwrap();
    let link = dir.path().join("linkdir");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let out = dir.path().join("out.txt");
    let err = match Extractor::new(
        &link,
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        false,
        no_interrupt(),
    ) {
        Err(e) => e,
        Ok(_) => panic!("expected symlink path to fail in new"),
    };
    assert!(err.to_string().contains("is a symlink"));
}

#[test]
fn debug_emits_processing_trail() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let mut ext = Extractor::new(
        sample.join("app.py"),
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        false,
        None,
        false,
        false,
        true,
        no_interrupt(),
    )
    .unwrap();
    ext.run().unwrap();

    let text = fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("processing:\n"));
    assert!(text.contains("\tapp.py\n"));
}

#[test]
fn skips_undecodable_file_and_continues() {
    for debug in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        write_good_and_bad(dir.path()).unwrap();
        let out = dir.path().join("out.txt");
        let path = dir.path().to_str().unwrap();
        let out_s = out.to_str().unwrap();
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"));
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
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
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

// --- recursion ---

#[test]
fn dir_walk_order_files_then_dirs_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in ["c.py", "a.py", "b.py"] {
        fs::write(root.join(name), format!("# TODO: {name}\n")).unwrap();
    }
    let nested = root.join("d");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("z.py"), "# TODO: nested\n").unwrap();
    let out = root.join("out.txt");
    let text = run_extract_with(root, &out, None, false, false, true, None).unwrap();
    assert!(text.find("TODO: a.py").unwrap() < text.find("TODO: b.py").unwrap());
    assert!(text.find("TODO: b.py").unwrap() < text.find("TODO: c.py").unwrap());
    assert!(text.find("TODO: c.py").unwrap() < text.find("TODO: nested").unwrap());
}

#[test]
fn r_finds_nested() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(&sample, &out, None, false, false, true, None).unwrap();
    assert!(text.contains("TODO: deep"));
    assert!(text.contains("TODO: deeper"));
}

#[test]
fn r_m_limits_depth() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(&sample, &out, None, false, false, true, Some(1)).unwrap();
    assert!(text.contains("TODO: deep"));
    assert!(!text.contains("TODO: deeper"));
}

#[test]
fn max_depth_rejects_non_positive() {
    let dir = tempfile::tempdir().unwrap();
    // out omitted on purpose: bad max_depth must fail before default TODO is created
    let err = match Extractor::new(
        dir.path(),
        None,
        false,
        None,
        false,
        false,
        None,
        false,
        false,
        false,
        Some(0),
        false,
        false,
        false,
        no_interrupt(),
    ) {
        Err(e) => e,
        Ok(_) => panic!("expected max_depth 0 to fail"),
    };
    assert!(
        err.to_string()
            .contains("max_depth must be a positive integer")
    );
}

#[test]
fn symlink_not_followed_dir() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("via_link.py"), "# TODO: through-symlink\n").unwrap();

    let scan = dir.path().join("scan");
    fs::create_dir(&scan).unwrap();
    fs::write(scan.join("top.py"), "# TODO: top\n").unwrap();
    std::os::unix::fs::symlink(&outside, scan.join("linkdir")).unwrap();

    let out = scan.join("out.txt");
    let text = run_extract_with(&scan, &out, None, false, false, true, None).unwrap();
    assert!(text.contains("TODO: top"));
    assert!(!text.contains("TODO: through-symlink"));
}

#[test]
fn s_writes_to_stdout_only() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
        .args([sample.join("app.py").to_str().unwrap(), "-s"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
    assert!(!sample.join("TODO").exists());
}

#[test]
fn tee_writes_stdout_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    // use the binary so tee stdout is captured (not mixed into the test runner)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
        .args([
            sample.join("app.py").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--tee",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&out).unwrap(), APP_OUT);
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
}

#[test]
fn tee_without_out_uses_todo() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
        .current_dir(&sample)
        .args(["app.py", "--tee"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(sample.join("TODO")).unwrap(), APP_OUT);
    assert_eq!(String::from_utf8_lossy(&output.stdout), APP_OUT);
}

#[test]
fn tee_survives_broken_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
        .args([
            sample.join("app.py").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--tee",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    // tee keeps writing the file; broken stdout is not a hard failure for tee
    assert!(status.success() || status.code() == Some(1));
    assert_eq!(fs::read_to_string(&out).unwrap(), APP_OUT);
}

#[test]
fn does_not_rescan_output_file() {
    // abs only: relative form needs chdir and races under parallel cargo test
    let stale = "======================\nstale.py\n\tline 1: # TODO: from previous run\n";
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("TODO");
    fs::write(&out, stale).unwrap();

    let mut ext = Extractor::new(
        &sample,
        None,
        false,
        None,
        false,
        false,
        Some(out.to_str().unwrap()),
        false,
        false,
        true,
        None,
        false,
        false,
        false,
        no_interrupt(),
    )
    .unwrap();
    ext.run().unwrap();
    let text = fs::read_to_string(&out).unwrap();
    assert!(!text.contains("TODO: from previous run"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn stdout_has_no_out_skip() {
    let dir = tempfile::tempdir().unwrap();
    let todo = dir.path().join("TODO");
    fs::write(&todo, "# TODO: keep scanning me\n").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tdx"))
        .args([dir.path().to_str().unwrap(), "-s"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("TODO: keep scanning me"));
}

// --- excludes / hidden / gitignore ---

#[test]
fn x_skips_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample,
        &out,
        None,
        false,
        Some(vec!["skip_me.py".into(), "vendor/".into()]),
        false,
        false,
        false,
        false,
        true,
        None,
    )
    .unwrap();
    assert!(!text.contains("TODO: skip"));
    assert!(!text.contains("TODO: vendored"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn skips_hidden_and_dunder() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(&sample, &out, None, false, false, true, None).unwrap();
    assert!(!text.contains("TODO: hidden"));
    assert!(!text.contains("TODO: dunder"));
}

#[test]
fn h_includes_hidden_and_dunder() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample, &out, None, false, None, true, false, false, false, true, None,
    )
    .unwrap();
    assert!(text.contains("TODO: hidden"));
    assert!(text.contains("TODO: dunder"));
}

#[test]
fn g_applies_gitignore_and_default_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample, &out, None, false, None, false, true, false, false, true, None,
    )
    .unwrap();
    assert!(!text.contains("TODO: vendored"));
    assert!(!text.contains("TODO: skip"));
    assert!(text.contains("TODO: main"));
    assert!(!text.contains("TODO: hidden"));
}

#[test]
fn g_with_h_includes_hidden_but_keeps_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample, &out, None, false, None, true, true, false, false, true, None,
    )
    .unwrap();
    assert!(text.contains("TODO: hidden"));
    assert!(!text.contains("TODO: vendored"));
    // .venv not in .gitignore; .* default off via -H
    assert!(text.contains("TODO: venv"));
}

#[test]
fn g_with_x_bang_reincludes_vendor() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample,
        &out,
        None,
        false,
        Some(vec!["!vendor/".into(), "!vendor/**".into()]),
        false,
        true,
        false,
        false,
        true,
        None,
    )
    .unwrap();
    assert!(text.contains("TODO: vendored"));
}

#[test]
fn g_missing_gitignore_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let bare = sample.join("bare");
    let out = bare.join("out.txt");
    let text = run_extract_full(
        &bare, &out, None, false, None, false, true, false, false, false, None,
    )
    .unwrap();
    assert!(text.contains("TODO: bare"));
}

#[test]
fn file_path_g_is_noop_but_still_processes() {
    // -g only loads .gitignore for directory scans -> explicit file PATH is always processed
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_full(
        &sample.join("app.py"),
        &out,
        None,
        false,
        None,
        false,
        true,
        false,
        false,
        false,
        None,
    )
    .unwrap();
    assert!(text.contains("TODO: main"));
}
