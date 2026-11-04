use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
};

use todex::{OutputDest, Writer};

#[test]
fn out_path_truncated_on_init() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    fs::write(&out, "stale\n").unwrap();

    let mut w = Writer::new(
        OutputDest::file(out.to_str()),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    w.close().unwrap();

    assert_eq!(fs::read_to_string(&out).unwrap(), "");
}
