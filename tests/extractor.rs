mod common;

use std::fs;

use common::{
    APP_OUT, RunOpts, copy_sample, extractor, file_writer, no_interrupt, run, write_abc_tree,
    write_good_and_bad,
};
use todex::{OutputDest, Writer};

#[test]
fn default_scan() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(&sample.join("app.py"), &out, RunOpts::default()).unwrap();
    assert!(text.contains("TODO: main"));
    assert!(text.contains("FIXME: bug"));
    assert!(!text.contains("WARN: careful"));
}

#[test]
fn t_adds_token_and_keeps_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample.join("app.py"),
        &out,
        RunOpts {
            tokens: Some(vec!["WARN".into()]),
            ..Default::default()
        },
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
    let text = run(
        &sample.join("app.py"),
        &out,
        RunOpts {
            tokens: Some(vec!["WARN".into()]),
            ignore_default: true,
            ..Default::default()
        },
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
    let text = run(
        &sample.join("custom_tokens.py"),
        &out,
        RunOpts {
            tokens: Some(vec![
                "NOTE".into(),
                "!!!URGENT!!!".into(),
                "@FIXME".into(),
                "中文标记".into(),
            ]),
            ignore_default: true,
            ..Default::default()
        },
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

    let ext = extractor(sample.join("app.py"), RunOpts::default()).unwrap();
    let mut writer = Writer::new(OutputDest::file(reports.to_str()), no_interrupt()).unwrap();
    ext.run(&mut writer).unwrap();

    assert_eq!(fs::read_to_string(reports.join("TODO")).unwrap(), APP_OUT);
}

#[test]
fn omitted_out_defaults_to_todo() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&sample).unwrap();

    let result = (|| -> todex::Result<_> {
        let ext = extractor("app.py", RunOpts::default())?;
        let mut writer = Writer::new(OutputDest::file(None), no_interrupt())?;
        ext.run(&mut writer)?;
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
        assert_eq!(run(&src, &out, RunOpts::default()).unwrap(), "");
    }
}

#[test]
fn o_equals_scanned_file_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.py");
    fs::write(&src, "# TODO: will be truncated\n").unwrap();
    assert_eq!(run(&src, &src, RunOpts::default()).unwrap(), "");
}

#[test]
fn zero_hit_scan_truncates_stale_out() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("empty.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "print(1)\n").unwrap();
    fs::write(&out, "stale from previous run\n").unwrap();
    assert_eq!(run(&src, &out, RunOpts::default()).unwrap(), "");
}

#[test]
fn missing_path_raises() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let out = dir.path().join("out.txt");
    let result = extractor(&missing, RunOpts::default());
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("not found"));
    assert!(!out.exists());
}

#[test]
fn c_omits_snippet_body() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample.join("snippet.py"),
        &out,
        RunOpts {
            concise: true,
            ..Default::default()
        },
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
    let text = run(&sample.join("snippet.py"), &out, RunOpts::default()).unwrap();
    assert!(text.contains("return 1"));
}

#[test]
fn warn_snippet_count() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("warn.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "# WARN{2} check this\nx = 1\ny = 2\n").unwrap();
    let text = run(
        &src,
        &out,
        RunOpts {
            tokens: Some(vec!["WARN".into()]),
            ignore_default: true,
            ..Default::default()
        },
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
    let text = run(&src, &out, RunOpts::default()).unwrap();
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
            let ext = extractor(
                &src,
                RunOpts {
                    concise,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut writer = file_writer(&out).unwrap();
            let err = ext.run(&mut writer).unwrap_err();
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
    let text = run(&src, &out, RunOpts::default()).unwrap();
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
    let text = run(
        &target,
        &out,
        RunOpts {
            full_path: true,
            ..Default::default()
        },
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
    let text = run(&sample, &out, RunOpts::default()).unwrap();
    assert!(!text.contains("TODO: deep"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn dir_walk_order_depth0_files_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write_abc_tree(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run(dir.path(), &out, RunOpts::default()).unwrap();
    assert!(text.find("TODO: a.py").unwrap() < text.find("TODO: b.py").unwrap());
    assert!(text.find("TODO: b.py").unwrap() < text.find("TODO: c.py").unwrap());
    assert!(!text.contains("TODO: nested"));
}

#[test]
fn symlink_not_followed() {
    for (link_name, target_is_dir, recursive, hidden) in [
        ("link.py", false, false, "TODO: via-file-link"),
        ("linkdir", true, true, "TODO: through-symlink"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let target = if target_is_dir {
            fs::write(outside.join("via_link.py"), format!("# {hidden}\n")).unwrap();
            outside.clone()
        } else {
            let target = outside.join("target.py");
            fs::write(&target, format!("# {hidden}\n")).unwrap();
            target
        };

        let scan = dir.path().join("scan");
        fs::create_dir(&scan).unwrap();
        fs::write(scan.join("top.py"), "# TODO: top\n").unwrap();
        std::os::unix::fs::symlink(&target, scan.join(link_name)).unwrap();

        let out = scan.join("out.txt");
        let text = run(
            &scan,
            &out,
            RunOpts {
                recursive,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(text.contains("TODO: top"), "link={link_name}");
        assert!(!text.contains(hidden), "link={link_name}");
    }
}

#[test]
fn symlink_path_raises() {
    for target_is_dir in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let target = dir
            .path()
            .join(if target_is_dir { "dir" } else { "file.py" });
        if target_is_dir {
            fs::create_dir(&target).unwrap();
            fs::write(target.join("x.py"), "# TODO: x\n").unwrap();
        } else {
            fs::write(&target, "# TODO: x\n").unwrap();
        }
        let link = dir
            .path()
            .join(if target_is_dir { "linkdir" } else { "link.py" });
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let result = extractor(&link, RunOpts::default());
        assert!(result.is_err(), "target_is_dir={target_is_dir}");
        assert!(result.err().unwrap().to_string().contains("is a symlink"));
    }
}

#[test]
fn debug_emits_processing_trail() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        &sample.join("app.py"),
        &out,
        RunOpts {
            debug: true,
            ..Default::default()
        },
    )
    .unwrap();
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

#[test]
fn dir_walk_order_files_then_dirs_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write_abc_tree(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        dir.path(),
        &out,
        RunOpts {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.find("TODO: a.py").unwrap() < text.find("TODO: b.py").unwrap());
    assert!(text.find("TODO: b.py").unwrap() < text.find("TODO: c.py").unwrap());
    assert!(text.find("TODO: c.py").unwrap() < text.find("TODO: nested").unwrap());
}

#[test]
fn r_finds_nested() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        RunOpts {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: deep"));
    assert!(text.contains("TODO: deeper"));
}

#[test]
fn r_m_limits_depth() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        RunOpts {
            recursive: true,
            max_depth: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: deep"));
    assert!(!text.contains("TODO: deeper"));
}

#[test]
fn max_depth_rejects_non_positive() {
    let dir = tempfile::tempdir().unwrap();
    let result = extractor(
        dir.path(),
        RunOpts {
            max_depth: Some(0),
            ..Default::default()
        },
    );
    assert!(result.is_err());
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("max_depth must be a positive integer")
    );
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
    // tee still finishes the file; broken stdout is not always a hard failure
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

    let text = run(
        &sample,
        &out,
        RunOpts {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
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

#[test]
fn x_skips_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        RunOpts {
            exclude: Some(vec!["skip_me.py".into(), "vendor/".into()]),
            recursive: true,
            ..Default::default()
        },
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
    let text = run(
        &sample,
        &out,
        RunOpts {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!text.contains("TODO: hidden"));
    assert!(!text.contains("TODO: dunder"));
}

#[test]
fn h_includes_hidden_and_dunder() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        RunOpts {
            include_hidden: true,
            recursive: true,
            ..Default::default()
        },
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
    let text = run(
        &sample,
        &out,
        RunOpts {
            use_gitignore: true,
            recursive: true,
            ..Default::default()
        },
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
    let text = run(
        &sample,
        &out,
        RunOpts {
            include_hidden: true,
            use_gitignore: true,
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: hidden"));
    assert!(!text.contains("TODO: vendored"));
    assert!(text.contains("TODO: venv"));
}

#[test]
fn g_with_x_bang_reincludes_vendor() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        RunOpts {
            exclude: Some(vec!["!vendor/".into(), "!vendor/**".into()]),
            use_gitignore: true,
            recursive: true,
            ..Default::default()
        },
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
    let text = run(
        &bare,
        &out,
        RunOpts {
            use_gitignore: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: bare"));
}

#[test]
fn file_path_g_is_noop_but_still_processes() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample.join("app.py"),
        &out,
        RunOpts {
            use_gitignore: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: main"));
}
