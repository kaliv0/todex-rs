mod common;

use std::fs;

use common::{
    APP_OUT, CwdGuard, assert_appears_in_order, copy_file, copy_sample, file_writer, no_interrupt,
    run, run_default, write_abc_tree,
};
use todex::{Extractor, ExtractorConfig, OutputDest, Writer};

#[test]
fn token_opts_default_t_and_i() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("out.txt");
    for (label, config, expect_todo, expect_warn) in [
        ("default", ExtractorConfig::default(), true, false),
        (
            "t_adds",
            ExtractorConfig {
                tokens: Some(vec!["WARN".into()]),
                ..Default::default()
            },
            true,
            true,
        ),
        (
            "i_with_t",
            ExtractorConfig {
                tokens: Some(vec!["WARN".into()]),
                ignore_default: true,
                ..Default::default()
            },
            false,
            true,
        ),
    ] {
        let text = run(&src, &out, config).unwrap();
        assert_eq!(text.contains("TODO: main"), expect_todo, "{label}");
        assert_eq!(text.contains("WARN: careful"), expect_warn, "{label}");
        if label == "default" {
            assert!(text.contains("FIXME: bug"), "{label}");
        }
    }
}

#[test]
fn custom_tokens_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "custom_tokens.py").unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        &src,
        &out,
        ExtractorConfig {
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
    let src = copy_file(dir.path(), "app.py").unwrap();
    let reports = dir.path().join("reports");
    fs::create_dir(&reports).unwrap();

    let ext = Extractor::new(&src, ExtractorConfig::default()).unwrap();
    let mut writer = Writer::new(OutputDest::file(reports.to_str()), no_interrupt()).unwrap();
    ext.run(&mut writer).unwrap();

    assert_eq!(fs::read_to_string(reports.join("TODO")).unwrap(), APP_OUT);
}

#[test]
fn omitted_out_defaults_to_todo() {
    let dir = tempfile::tempdir().unwrap();
    copy_file(dir.path(), "app.py").unwrap();
    let _cwd = CwdGuard::enter(dir.path());

    let ext = Extractor::new("app.py", ExtractorConfig::default()).unwrap();
    let mut writer = Writer::new(OutputDest::file(None), no_interrupt()).unwrap();
    ext.run(&mut writer).unwrap();
    assert_eq!(fs::read_to_string("TODO").unwrap(), APP_OUT);
}

#[test]
fn zero_token_file_empty_out() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in [("empty.py", ""), ("nl.py", "\n")] {
        let src = dir.path().join(name);
        let out = dir.path().join(format!("{name}.out"));
        fs::write(&src, content).unwrap();
        assert_eq!(run_default(&src, &out).unwrap(), "");
    }
}

#[test]
fn o_equals_scanned_file_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.py");
    fs::write(&src, "# TODO: will be truncated\n").unwrap();
    assert_eq!(run_default(&src, &src).unwrap(), "");
}

#[test]
fn zero_hit_scan_truncates_stale_out() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("empty.py");
    let out = dir.path().join("out.txt");
    fs::write(&src, "print(1)\n").unwrap();
    fs::write(&out, "stale from previous run\n").unwrap();
    assert_eq!(run_default(&src, &out).unwrap(), "");
}

#[test]
fn missing_path_raises() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let out = dir.path().join("out.txt");
    let Err(err) = Extractor::new(&missing, ExtractorConfig::default()) else {
        panic!("expected error for missing path");
    };
    assert!(err.to_string().contains("not found"));
    assert!(!out.exists());
}

#[test]
fn snippet_concise_vs_full() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "snippet.py").unwrap();
    let out = dir.path().join("out.txt");
    for (label, concise, expect_body) in [("concise", true, false), ("full", false, true)] {
        let text = run(
            &src,
            &out,
            ExtractorConfig {
                concise,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(text.contains("FIXME"), "{label}");
        assert_eq!(text.contains("return 1"), expect_body, "{label}");
    }
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
        ExtractorConfig {
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
    let text = run_default(&src, &out).unwrap();
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
            let ext = Extractor::new(
                &src,
                ExtractorConfig {
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
    let text = run_default(&src, &out).unwrap();
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
    let target = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        &target,
        &out,
        ExtractorConfig {
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
    let text = run_default(&sample, &out).unwrap();
    assert!(!text.contains("TODO: deep"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn dir_walk_order_depth0_files_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write_abc_tree(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run_default(dir.path(), &out).unwrap();
    assert_appears_in_order(&text, &["TODO: a.py", "TODO: b.py", "TODO: c.py"]);
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
            ExtractorConfig {
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
        let Err(err) = Extractor::new(&link, ExtractorConfig::default()) else {
            panic!("expected error for symlink path target_is_dir={target_is_dir}");
        };
        assert!(
            err.to_string().contains("is a symlink"),
            "target_is_dir={target_is_dir} err={err}"
        );
    }
}

#[test]
fn debug_emits_processing_trail() {
    let dir = tempfile::tempdir().unwrap();
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        &src,
        &out,
        ExtractorConfig {
            debug: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.starts_with("processing:\n"));
    assert!(text.contains("\tapp.py\n"));
}

#[test]
fn dir_walk_order_files_then_dirs_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write_abc_tree(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        dir.path(),
        &out,
        ExtractorConfig {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_appears_in_order(
        &text,
        &["TODO: a.py", "TODO: b.py", "TODO: c.py", "TODO: nested"],
    );
}

#[test]
fn r_finds_nested() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        ExtractorConfig {
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
        ExtractorConfig {
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
    let Err(err) = Extractor::new(
        dir.path(),
        ExtractorConfig {
            max_depth: Some(0),
            ..Default::default()
        },
    ) else {
        panic!("expected error for non-positive max_depth");
    };
    assert!(
        err.to_string()
            .contains("max_depth must be a positive integer")
    );
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
        ExtractorConfig {
            recursive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!text.contains("TODO: from previous run"));
    assert!(text.contains("TODO: main"));
}

#[test]
fn x_skips_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        ExtractorConfig {
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
fn h_and_default_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    for (label, include_hidden, expect_hidden) in [("default", false, false), ("h", true, true)] {
        let text = run(
            &sample,
            &out,
            ExtractorConfig {
                include_hidden,
                recursive: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(text.contains("TODO: hidden"), expect_hidden, "{label}");
        assert_eq!(text.contains("TODO: dunder"), expect_hidden, "{label}");
    }
}

#[test]
fn g_applies_gitignore_and_default_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run(
        &sample,
        &out,
        ExtractorConfig {
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
        ExtractorConfig {
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
        ExtractorConfig {
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
        ExtractorConfig {
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
    let src = copy_file(dir.path(), "app.py").unwrap();
    let out = dir.path().join("out.txt");
    let text = run(
        &src,
        &out,
        ExtractorConfig {
            use_gitignore: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(text.contains("TODO: main"));
}
