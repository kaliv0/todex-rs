#![allow(dead_code)] // kept here so we don't switch to common/mod.rs layout

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{Arc, Mutex, MutexGuard, atomic::AtomicBool},
};

use todex::{Extractor, ExtractorConfig, OutputDest, Result, Writer};

pub fn tdx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tdx"))
}

pub fn run_tdx(args: &[&str]) -> Output {
    tdx().args(args).output().expect("run tdx")
}

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Holds process cwd for one test and restores it on drop (serialized via `CWD_LOCK`)
pub struct CwdGuard {
    prev: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CwdGuard {
    pub fn enter(dir: &Path) -> Self {
        let lock = CWD_LOCK.lock().unwrap();
        let prev = env::current_dir().unwrap();
        env::set_current_dir(dir).unwrap();
        Self { prev, _lock: lock }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
    }
}

pub fn no_interrupt() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

pub const APP_OUT: &str =
    "======================\napp.py\n\tline 1: # TODO: main\n\tline 2: # FIXME: bug\n";

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

pub fn copy_file(dest: &Path, name: &str) -> io::Result<PathBuf> {
    let dest_file = dest.join(name);
    if let Some(parent) = dest_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(testdata_root().join(name), &dest_file)?;
    Ok(dest_file)
}

pub fn file_writer(out: &Path) -> Result<Writer> {
    Writer::new(OutputDest::file(out.to_str()), no_interrupt())
}

pub fn run(path: &Path, out: &Path, config: ExtractorConfig) -> Result<String> {
    let ext = Extractor::new(path, config)?;
    let mut writer = file_writer(out)?;
    ext.run(&mut writer)?;
    if out.is_file() {
        Ok(fs::read_to_string(out)?)
    } else {
        Ok(String::new())
    }
}

pub fn run_default(path: &Path, out: &Path) -> Result<String> {
    run(path, out, ExtractorConfig::default())
}

pub fn assert_appears_in_order(text: &str, needles: &[&str]) {
    let mut from = 0;
    for needle in needles {
        match text[from..].find(needle) {
            Some(i) => from += i + needle.len(),
            None => panic!("expected {needle:?} after offset {from} in:\n{text}"),
        }
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
