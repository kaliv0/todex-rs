// shared fixtures and helpers for integration tests

use std::path::{Path, PathBuf};
use std::{fs, io};

use todex::extractor::Extractor;

pub const APP_OUT: &str =
    "======================\napp.py\n\tline 1: # TODO: main\n\tline 2: # FIXME: bug\n";

pub fn testdata_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata")
}

pub fn copy_sample(dest: &Path) -> io::Result<PathBuf> {
    let project = dest.join("project");
    copy_dir_all(&testdata_root(), &project)?;
    // paths matching ignore rules never land in git — materialize them for the scan
    // (same as Python conftest sample fixture)
    fs::write(project.join("skip_me.py"), "# TODO: skip\n")?;
    fs::create_dir_all(project.join("vendor"))?;
    fs::write(project.join("vendor/lib.py"), "# TODO: vendored\n")?;
    fs::create_dir_all(project.join(".venv"))?;
    fs::write(project.join(".venv/x.py"), "# TODO: venv\n")?;
    Ok(project)
}

pub fn run_extract(path: &Path, out: &Path) -> io::Result<String> {
    run_extract_with(path, out, None, false, false, false, None)
}

pub fn run_extract_with(
    path: &Path,
    out: &Path,
    tokens: Option<Vec<String>>,
    ignore_default: bool,
    concise: bool,
    recursive: bool,
    max_depth: Option<usize>,
) -> io::Result<String> {
    run_extract_full(
        path,
        out,
        tokens,
        ignore_default,
        None,
        false,
        false,
        false,
        concise,
        recursive,
        max_depth,
    )
}

pub fn run_extract_full(
    path: &Path,
    out: &Path,
    tokens: Option<Vec<String>>,
    ignore_default: bool,
    exclude: Option<Vec<String>>,
    include_hidden: bool,
    use_gitignore: bool,
    full_path: bool,
    concise: bool,
    recursive: bool,
    max_depth: Option<usize>,
) -> io::Result<String> {
    let mut ext = Extractor::new(
        path,
        tokens,
        ignore_default,
        exclude,
        include_hidden,
        use_gitignore,
        Some(out.to_str().unwrap()),
        full_path,
        concise,
        recursive,
        max_depth,
        false,
        false,
        false,
    )?;
    ext.run()?;
    if out.is_file() {
        fs::read_to_string(out)
    } else {
        Ok(String::new())
    }
}

pub fn write_good_and_bad(dir: &Path) -> io::Result<()> {
    fs::write(dir.join("good.py"), "# TODO: keep me\n")?;
    fs::write(dir.join("bad.bin"), b"\xff\xfe TODO: binary\n")?;
    Ok(())
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
