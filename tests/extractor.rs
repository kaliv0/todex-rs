mod common;

use std::fs;

use common::{copy_sample, run_extract, APP_OUT};
use todexide::extractor::Extractor;

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
fn o_dir_writes_todo_inside() {
    let dir = tempfile::tempdir().unwrap();
    let sample = copy_sample(dir.path()).unwrap();
    let reports = sample.join("reports");
    fs::create_dir(&reports).unwrap();

    let mut ext = Extractor::new(sample.join("app.py"), Some(reports.to_str().unwrap())).unwrap();
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
        let mut ext = Extractor::new("app.py", None)?;
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
        let mut ext = Extractor::new(&src, Some(out.to_str().unwrap())).unwrap();
        ext.run().unwrap();
        assert_eq!(fs::read_to_string(&out).unwrap(), "");
    }
}

#[test]
fn o_equals_scanned_file_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.py");
    fs::write(&src, "# TODO: will be truncated\n").unwrap();

    let mut ext = Extractor::new(&src, Some(src.to_str().unwrap())).unwrap();
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

    let mut ext = Extractor::new(&src, Some(out.to_str().unwrap())).unwrap();
    ext.run().unwrap();

    assert_eq!(fs::read_to_string(&out).unwrap(), "");
}

#[test]
fn missing_path_raises() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let out = dir.path().join("out.txt");
    let err = match Extractor::new(&missing, Some(out.to_str().unwrap())) {
        Err(e) => e,
        Ok(_) => panic!("expected missing path to fail in new"),
    };
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(!out.exists());
}
