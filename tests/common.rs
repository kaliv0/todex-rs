#![allow(dead_code)]

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use todex::{Extractor, Result, Writer};

pub fn no_interrupt() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

pub const APP_OUT: &str =
    "======================\napp.py\n\tline 1: # TODO: main\n\tline 2: # FIXME: bug\n";

#[derive(Default)]
pub struct RunOpts {
    pub tokens: Option<Vec<String>>,
    pub ignore_default: bool,
    pub exclude: Option<Vec<String>>,
    pub include_hidden: bool,
    pub use_gitignore: bool,
    pub full_path: bool,
    pub concise: bool,
    pub recursive: bool,
    pub max_depth: Option<usize>,
    pub debug: bool,
}

pub fn testdata_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata")
}

pub fn copy_sample(dest: &Path) -> io::Result<PathBuf> {
    let project = dest.join("project");
    copy_dir_all(&testdata_root(), &project)?;
    // paths matching ignore rules never land in git — materialize them for the scan
    fs::write(project.join("skip_me.py"), "# TODO: skip\n")?;
    fs::create_dir_all(project.join("vendor"))?;
    fs::write(project.join("vendor/lib.py"), "# TODO: vendored\n")?;
    fs::create_dir_all(project.join(".venv"))?;
    fs::write(project.join(".venv/x.py"), "# TODO: venv\n")?;
    Ok(project)
}

pub fn extractor(path: impl AsRef<Path>, opts: RunOpts) -> Result<Extractor> {
    Extractor::new(
        path,
        opts.tokens,
        opts.ignore_default,
        opts.exclude,
        opts.include_hidden,
        opts.use_gitignore,
        opts.full_path,
        opts.concise,
        opts.recursive,
        opts.max_depth,
        opts.debug,
    )
}

pub fn file_writer(out: &Path) -> Result<Writer> {
    Writer::new(Some(out.to_str().unwrap()), false, false, no_interrupt())
}

pub fn run(path: &Path, out: &Path, opts: RunOpts) -> Result<String> {
    let ext = extractor(path, opts)?;
    let mut writer = file_writer(out)?;
    ext.run(&mut writer)?;
    if out.is_file() {
        Ok(fs::read_to_string(out)?)
    } else {
        Ok(String::new())
    }
}

pub fn write_good_and_bad(dir: &Path) -> io::Result<()> {
    fs::write(dir.join("good.py"), "# TODO: keep me\n")?;
    fs::write(dir.join("bad.bin"), b"\xff\xfe TODO: binary\n")?;
    Ok(())
}

pub fn write_abc_tree(root: &Path) -> io::Result<()> {
    for name in ["c.py", "a.py", "b.py"] {
        fs::write(root.join(name), format!("# TODO: {name}\n"))?;
    }
    let nested = root.join("d");
    fs::create_dir(&nested)?;
    fs::write(nested.join("z.py"), "# TODO: nested\n")?;
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
