mod common;

use std::fs;

use common::file_writer;

#[test]
fn out_path_truncated_on_init() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    fs::write(&out, "stale\n").unwrap();

    let mut w = file_writer(&out).unwrap();
    w.close().unwrap();

    assert_eq!(fs::read_to_string(&out).unwrap(), "");
}
