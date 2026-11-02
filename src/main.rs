use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, bail};
use clap::Parser;
use todex::{
    cli::Cli,
    extractor::Extractor,
    writer::{Interrupted, StopExtraction},
};

const PROG: &str = env!("CARGO_BIN_NAME");

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) if is_interrupted(&e) => std::process::exit(130),
        Err(e) if is_pipe_stop(&e) => std::process::exit(0), // 0 rg-style or 141 Unix-style
        Err(e) => {
            eprintln!("{PROG}: error: {e}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // validate dependent flags
    if cli.ignore_default && cli.tokens.is_none() {
        bail!("-i/--ignore-default flag requires -t/--tokens");
    }
    if cli.max_depth.is_some() && !cli.recursive {
        bail!("-m/--max-depth flag requires -r/--recursive");
    }
    if cli.stdout && (cli.out.is_some() || cli.tee) {
        bail!("shouldn't mix --stdout and --out or --tee flags");
    }

    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    })?;

    let mut extractor = Extractor::new(
        &cli.path,
        cli.tokens,
        cli.ignore_default,
        cli.exclude,
        cli.include_hidden,
        cli.use_gitignore,
        cli.out.as_deref(),
        cli.full_path,
        cli.concise,
        cli.recursive,
        cli.max_depth,
        cli.stdout,
        cli.tee,
        cli.debug,
        interrupted,
    )?;

    extractor.run()
}

fn is_interrupted(e: &anyhow::Error) -> bool {
    // Ctrl-C
    e.downcast_ref::<Interrupted>().is_some()
}

fn is_pipe_stop(e: &anyhow::Error) -> bool {
    // pipe (`tdx | head`)
    e.downcast_ref::<StopExtraction>().is_some()
        || e.downcast_ref::<std::io::Error>()
            .is_some_and(|ioe| ioe.kind() == std::io::ErrorKind::BrokenPipe)
}
