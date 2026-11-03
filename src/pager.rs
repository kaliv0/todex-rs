use std::{
    env, io,
    io::Write,
    process::{Command, Stdio},
};

use crate::error::Result;

pub enum StdoutSink {
    None,
    Raw,
    Paged(String),
}

fn resolve_pager_cmd() -> Option<String> {
    env::var("MANPAGER")
        .or_else(|_| env::var("PAGER"))
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn page(text: &str) -> Result<()> {
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
    match io::stdout().write_all(text.as_bytes()) {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(Into::into),
    }
}
