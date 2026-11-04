//! Output sinks for extract hits.

use std::{
    fs::File,
    io::{self, BufWriter, IsTerminal, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    error::{Error, Result},
    pager::page,
};

/// Default out file name when -o is omitted.
const DEFAULT_OUT: &str = "TODO";

/// CLI-facing destination for extract output: file, stdout, or both (tee).
pub enum OutputDest {
    File(PathBuf),
    Stdout,
    Tee(PathBuf),
}

impl OutputDest {
    /// Out file -> default name `TODO`. If `name` is a dir, write `TODO` inside it.
    pub fn file(name: Option<&str>) -> Self {
        Self::File(Self::resolve_out_path(name))
    }

    /// Out file and stdout.
    pub fn tee(name: Option<&str>) -> Self {
        Self::Tee(Self::resolve_out_path(name))
    }

    fn resolve_out_path(name: Option<&str>) -> PathBuf {
        let mut path = PathBuf::from(name.unwrap_or(DEFAULT_OUT));
        if path.is_dir() {
            path = path.join(DEFAULT_OUT);
        }
        path
    }
}

/// Runtime stdout mode for an open [`Writer`]: off, direct, or TTY-buffered for a pager.
pub(crate) enum StdoutSink {
    None,
    Raw,
    Paged(String),
}

/// Writes extract output to a file, stdout, or both.
pub struct Writer {
    path: Option<PathBuf>,
    to_stdout: bool,
    file: Option<BufWriter<File>>,
    stdout: StdoutSink,
    interrupted: Arc<AtomicBool>,
}

impl Writer {
    /// Open a writer.
    pub fn new(dest: OutputDest, interrupted: Arc<AtomicBool>) -> Result<Self> {
        match dest {
            OutputDest::Stdout => Self::create(None, true, interrupted),
            OutputDest::File(path) => Self::create(Some(path), false, interrupted),
            OutputDest::Tee(path) => Self::create(Some(path), true, interrupted),
        }
    }

    /// Truncate the out file early and pick a stdout sink.
    fn create(
        path: Option<PathBuf>,
        to_stdout: bool,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Self> {
        let file = match &path {
            // truncate early out_file even if no tokens found
            Some(p) => Some(BufWriter::new(File::create(p)?)),
            None => None,
        };

        let stdout = if !to_stdout {
            StdoutSink::None
        } else if io::stdout().is_terminal() {
            StdoutSink::Paged(String::new())
        } else {
            StdoutSink::Raw
        };

        Ok(Self {
            path,
            to_stdout,
            file,
            stdout,
            interrupted,
        })
    }

    /// True after an interrupt was requested.
    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    /// Out file path, if any.
    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Write a file header block.
    pub(crate) fn write_header(&mut self, path_name: &str) -> Result<()> {
        self.write(&format!("======================\n{path_name}\n"))
    }

    /// Write one hit line with its line number.
    pub(crate) fn write_entry(&mut self, lines_count: usize, line: &str) -> Result<()> {
        self.write(&format!("\tline {lines_count}: {line}"))
    }

    /// Write debug text to the active sinks.
    pub(crate) fn debug(&mut self, text: &str) -> Result<()> {
        self.write(text)
    }

    /// Write an error line to stderr or the pager buffer.
    pub(crate) fn error(&mut self, text: &str) -> Result<()> {
        if let StdoutSink::Paged(buf) = &mut self.stdout {
            buf.push_str(text);
        } else {
            eprint!("{text}");
        }
        Ok(())
    }

    /// Write text to the active sinks. Broken pipe may stop extraction.
    fn write(&mut self, text: &str) -> Result<()> {
        if self.interrupted() {
            return Err(Error::Interrupted);
        }

        if self.to_stdout {
            let stdout_result = match &mut self.stdout {
                StdoutSink::Raw => io::stdout()
                    .write_all(text.as_bytes())
                    .and_then(|_| io::stdout().flush()),
                StdoutSink::Paged(buf) => {
                    buf.push_str(text);
                    Ok(())
                }
                StdoutSink::None => Ok(()),
            };

            if let Err(e) = stdout_result {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    return Err(e.into());
                }

                self.to_stdout = false;
                if matches!(self.stdout, StdoutSink::Paged(_)) {
                    // drop pager so close() won't page() partial buffer
                    self.stdout = StdoutSink::None;
                }
                if self.path.is_none() {
                    return Err(Error::StopExtraction);
                }
            }
        }

        if let Some(file) = self.file.as_mut() {
            file.write_all(text.as_bytes())?;
            file.flush()?;
        }
        Ok(())
    }

    /// Flush sinks. Page buffered TTY stdout if needed.
    pub fn close(&mut self) -> Result<()> {
        if let StdoutSink::Paged(buf) = std::mem::replace(&mut self.stdout, StdoutSink::None) {
            page(&buf)?;
        }

        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        Ok(())
    }
}

////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, sync::Mutex};

    static PAGER_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_pager<T>(cmd: &str, f: impl FnOnce() -> T) -> T {
        let _guard = PAGER_ENV_LOCK.lock().unwrap();
        let prev_man = env::var("MANPAGER").ok();
        let prev_pager = env::var("PAGER").ok();
        // edition 2024: set_var / remove_var are unsafe
        unsafe {
            env::set_var("MANPAGER", cmd);
            env::remove_var("PAGER");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe {
            match prev_man {
                Some(v) => env::set_var("MANPAGER", v),
                None => env::remove_var("MANPAGER"),
            }
            match prev_pager {
                Some(v) => env::set_var("PAGER", v),
                None => env::remove_var("PAGER"),
            }
        }
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    /// Capture pager stdin to log -> silence pager stdout so cargo test stays clean.
    fn pager_to_file(log: &Path) -> String {
        format!("tee {} >/dev/null", log.display())
    }

    fn no_interrupt() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn paged_writer(path: Option<&Path>) -> Writer {
        let path = path.map(|p| p.to_path_buf());
        let file = path
            .as_ref()
            .map(|p| BufWriter::new(File::create(p).unwrap()));
        Writer {
            path,
            to_stdout: true,
            file,
            stdout: StdoutSink::Paged(String::new()),
            interrupted: no_interrupt(),
        }
    }

    #[test]
    fn non_tty_uses_raw_stdout() {
        if io::stdout().is_terminal() {
            return;
        }
        let w = Writer::new(OutputDest::Stdout, no_interrupt()).unwrap();
        assert!(matches!(w.stdout, StdoutSink::Raw));
    }

    #[test]
    fn tty_buffers_and_pages_on_close() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("paged.txt");
        with_pager(&pager_to_file(&log), || {
            let mut w = paged_writer(None);
            w.write_header("a.py").unwrap();
            w.write_entry(1, "# TODO: x\n").unwrap();
            assert!(!log.exists() || fs::read_to_string(&log).unwrap().is_empty());

            w.close().unwrap();
            assert_eq!(
                fs::read_to_string(&log).unwrap(),
                "======================\na.py\n\tline 1: # TODO: x\n"
            );
        });
    }

    #[test]
    fn tty_error_goes_to_pager_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("paged.txt");
        with_pager(&pager_to_file(&log), || {
            let mut w = paged_writer(None);
            w.error("boom\n").unwrap();
            w.close().unwrap();
            assert_eq!(fs::read_to_string(&log).unwrap(), "boom\n");
        });
    }

    #[test]
    fn non_tty_error_goes_to_stderr() {
        if io::stdout().is_terminal() {
            return;
        }
        let mut w = Writer::new(OutputDest::Stdout, no_interrupt()).unwrap();
        w.error("boom\n").unwrap();
    }

    #[test]
    fn tty_tee_pages_and_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("paged.txt");
        let out = dir.path().join("out.txt");
        with_pager(&pager_to_file(&log), || {
            let mut w = paged_writer(Some(&out));
            w.write_entry(1, "# TODO: x\n").unwrap();
            w.close().unwrap();

            assert_eq!(fs::read_to_string(&log).unwrap(), "\tline 1: # TODO: x\n");
            assert_eq!(fs::read_to_string(&out).unwrap(), "\tline 1: # TODO: x\n");
        });
    }

    #[test]
    fn page_does_not_mutate_less() {
        let before = env::var("LESS").ok();
        with_pager("cat >/dev/null", || {
            let mut w = paged_writer(None);
            w.write_entry(1, "# TODO: x\n").unwrap();
            w.close().unwrap();
        });
        assert_eq!(env::var("LESS").ok(), before);
    }
}
