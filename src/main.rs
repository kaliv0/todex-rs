use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use clap::Parser;
use todex::{cli::Cli, error::Result, extractor::Extractor};

const PROG: &str = env!("CARGO_BIN_NAME");

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) if e.is_interrupted() => std::process::exit(130),
        Err(e) if e.is_pipe_stop() => std::process::exit(0), // 0 rg-style or 141 Unix-style
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
        die("-i/--ignore-default flag requires -t/--tokens");
    }
    if cli.max_depth.is_some() && !cli.recursive {
        die("-m/--max-depth flag requires -r/--recursive");
    }
    if cli.stdout && (cli.out.is_some() || cli.tee) {
        die("shouldn't mix --stdout and --out or --tee flags");
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

fn die(msg: &str) -> ! {
    eprintln!("{PROG}: error: {msg}");
    std::process::exit(1);
}
