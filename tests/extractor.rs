mod common;

use std::fs;

use common::{APP_OUT, copy_sample, run_extract, run_extract_with, write_good_and_bad};
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
        Some(reports.to_str().unwrap()),
        false,
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

    let result = (|| {
        let mut ext = Extractor::new("app.py", None, false, None, false)?;
        ext.run()?;
        fs::read_to_string("TODO")
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
        let mut ext =
            Extractor::new(&src, None, false, Some(out.to_str().unwrap()), false).unwrap();
        ext.run().unwrap();
        assert_eq!(fs::read_to_string(&out).unwrap(), "");
    }
}

#[test]
fn o_equals_scanned_file_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.py");
    fs::write(&src, "# TODO: will be truncated\n").unwrap();

    let mut ext = Extractor::new(&src, None, false, Some(src.to_str().unwrap()), false).unwrap();
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

    let mut ext = Extractor::new(&src, None, false, Some(out.to_str().unwrap()), false).unwrap();
    ext.run().unwrap();

    assert_eq!(fs::read_to_string(&out).unwrap(), "");
}

#[test]
fn missing_path_raises() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let out = dir.path().join("out.txt");
    let err = match Extractor::new(&missing, None, false, Some(out.to_str().unwrap()), false) {
        Err(e) => e,
        Ok(_) => panic!("expected missing path to fail in new"),
    };
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(!out.exists());
}

// --- concise / snippets ---

#[test]
fn c_omits_snippet_body() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let out = sample.join("out.txt");
    let text = run_extract_with(&sample.join("snippet.py"), &out, None, false, true).unwrap();
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
    let text = run_extract_with(&src, &out, Some(vec!["WARN".into()]), true, false).unwrap();
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
fn get_snippet_count_after_token() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    let ext = Extractor::new(dir.path(), None, false, Some(out.to_str().unwrap()), false).unwrap();
    for (line, expected) in [("# TODO: x", 1usize), ("# TODO{3} x", 3)] {
        let token_end = line.find("TODO").unwrap() + "TODO".len();
        assert_eq!(
            ext.get_snippet_count_after_token(line, token_end, 1)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn invalid_snippet_raises() {
    let dir = tempfile::tempdir().unwrap();
    for content in ["# TODO{3 no close\n", "# TODO{abc}\n", "# TODO{-1}\n"] {
        for concise in [false, true] {
            let src = dir.path().join("bad.py");
            let out = dir.path().join("out.txt");
            fs::write(&src, content).unwrap();
            let mut ext =
                Extractor::new(&src, None, false, Some(out.to_str().unwrap()), concise).unwrap();
            let err = ext.run().unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
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
fn skips_undecodable_file_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    write_good_and_bad(dir.path()).unwrap();
    let out = dir.path().join("out.txt");
    let text = run_extract(dir.path(), &out).unwrap();
    assert!(text.contains("TODO: keep me"));
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
    let text = run_extract(&src, &out).unwrap();
    assert!(text.contains("TODO: first"));
}
