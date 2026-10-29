// shared fixtures and helpers for integration tests

use std::path::{Path, PathBuf};
use std::{fs, io};

use todexide::extractor::Extractor;

pub const APP_OUT: &str =
    "======================\napp.py\n\tline 1: # TODO: main\n\tline 2: # FIXME: bug\n";

pub fn testdata_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata")
}

pub fn copy_sample(dest: &Path) -> io::Result<PathBuf> {
    let project = dest.join("project");
    copy_dir_all(&testdata_root(), &project)?;
    Ok(project)
}

pub fn run_extract(path: &Path, out: &Path) -> io::Result<String> {
    let mut ext = Extractor::new(path, Some(out.to_str().unwrap()))?;
    ext.run()?;
    if out.is_file() {
        fs::read_to_string(out)
    } else {
        Ok(String::new())
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}
