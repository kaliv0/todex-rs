use std::{
    env,
    fs::File,
    io::{self, BufWriter, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct Writer {
    path: Option<PathBuf>,
    to_stdout: bool,
    file: Option<BufWriter<File>>,
    stdout: StdoutSink,
}

impl Writer {
    pub fn new(path: Option<PathBuf>, to_stdout: bool) -> std::io::Result<Self> {
        let file = path
            .as_ref()
            .map(|p| -> Result<_, std::io::Error> {
                //??why not std::io::Result
                Ok(BufWriter::new(File::create(p)?)) // truncate early out_file even if no tokens found
            })
            .transpose()?;

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
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn write_header(&mut self, path_name: &str) -> std::io::Result<()> {
        self.write(&format!("======================\n{path_name}\n"))
    }

    pub fn write_entry(&mut self, lines_count: usize, line: &str) -> std::io::Result<()> {
        self.write(&format!("\tline {lines_count}: {line}"))
    }

    pub fn error(&mut self, text: &str) -> std::io::Result<()> {
        if let StdoutSink::Paged(buf) = &mut self.stdout {
            buf.push_str(text);
        } else {
            eprint!("{text}");
        }
        Ok(())
    }

    fn write(&mut self, text: &str) -> std::io::Result<()> {
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
                    return Err(e);
                }

                self.to_stdout = false;
                if matches!(self.stdout, StdoutSink::Paged(_)) {
                    // drop pager so close() won't page() a partial buffer
                    self.stdout = StdoutSink::None;
                }
                if self.path.is_none() {
                    return Err(e);
                }
            }
        }

        if let Some(file) = self.file.as_mut() {
            file.write_all(text.as_bytes())?;
            file.flush()?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> std::io::Result<()> {
        if let StdoutSink::Paged(buf) = std::mem::replace(&mut self.stdout, StdoutSink::None) {
            page(&buf)?;
        }

        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        Ok(())
    }
}

enum StdoutSink {
    None,
    Raw,
    Paged(String),
}

fn resolve_pager_cmd() -> Option<String> {
    #[cfg(test)]
    if let Some(cmd) = tests::pager_override() {
        return Some(cmd);
    }
    env::var("MANPAGER")
        .or_else(|_| env::var("PAGER"))
        .ok()
        .filter(|s| !s.is_empty())
}

fn page(text: &str) -> std::io::Result<()> {
    let try_cmds = match resolve_pager_cmd() {
        Some(c) => vec![c],
        None => vec!["less".into(), "more".into()],
    };

    for c in &try_cmds {
        let Ok(mut child) = Command::new("sh")
            .args(["-c", c])
            .stdin(Stdio::piped())
            .spawn()
        else {
            continue;
        };

        if let Some(mut stdin) = child.stdin.take() {
            // if user quites early -> broken pipe is fine so we don't propagate error
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }

    // if no pager available
    io::stdout().write_all(text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, fs};

    thread_local! {
        static PAGER_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub(super) fn pager_override() -> Option<String> {
        PAGER_OVERRIDE.with(|c| c.borrow().clone())
    }

    fn with_pager<T>(cmd: &str, f: impl FnOnce() -> T) -> T {
        PAGER_OVERRIDE.with(|c| *c.borrow_mut() = Some(cmd.to_string()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        PAGER_OVERRIDE.with(|c| *c.borrow_mut() = None);
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    /// capture pager stdin to `log` -> silence pager stdout so cargo test stays clean.
    fn pager_to_file(log: &Path) -> String {
        format!("tee {} >/dev/null", log.display())
    }

    fn paged_writer(path: Option<PathBuf>) -> Writer {
        let file = path
            .as_ref()
            .map(|p| BufWriter::new(File::create(p).unwrap()));
        Writer {
            path,
            to_stdout: true,
            file,
            stdout: StdoutSink::Paged(String::new()),
        }
    }

    #[test]
    fn non_tty_uses_raw_stdout() {
        if io::stdout().is_terminal() {
            return;
        }
        let w = Writer::new(None, true).unwrap();
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
        let mut w = Writer::new(None, true).unwrap();
        w.error("boom\n").unwrap();
    }

    #[test]
    fn tty_tee_pages_and_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("paged.txt");
        let out = dir.path().join("out.txt");
        with_pager(&pager_to_file(&log), || {
            let mut w = paged_writer(Some(out.clone()));
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

    #[test]
    fn page_accepts_pager_with_args() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("paged.txt");
        with_pager(&pager_to_file(&log), || {
            let mut w = paged_writer(None);
            w.write_entry(1, "# TODO: x\n").unwrap();
            w.close().unwrap();
            assert!(
                fs::read_to_string(&log)
                    .unwrap()
                    .contains("\tline 1: # TODO: x\n")
            );
        });
    }
}
