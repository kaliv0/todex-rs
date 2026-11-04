use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use clap::Parser;
use todex::{Cli, Extractor, Result, Writer};

const PROG: &str = env!("CARGO_BIN_NAME");

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) if e.is_interrupted() => std::process::exit(130),
        Err(e) if e.is_pipe_stop() => std::process::exit(0), // 0 rg-style or 141 Unix-style
        Err(e) => die(e),
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    validate_flags(&cli);

    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    })?;

    let extractor = Extractor::new(
        &cli.path,
        cli.tokens,
        cli.ignore_default,
        cli.exclude,
        cli.include_hidden,
        cli.use_gitignore,
        cli.full_path,
        cli.concise,
        cli.recursive,
        cli.max_depth,
        cli.debug,
    )?;

    let mut writer = Writer::new(cli.out.as_deref(), cli.stdout, cli.tee, interrupted)?;

    extractor.run(&mut writer)
}

fn validate_flags(cli: &Cli) {
    if cli.ignore_default && cli.tokens.is_none() {
        die("-i/--ignore-default flag requires -t/--tokens");
    }
    if cli.max_depth.is_some() && !cli.recursive {
        die("-m/--max-depth flag requires -r/--recursive");
    }
    if cli.stdout && (cli.out.is_some() || cli.tee) {
        die("shouldn't mix --stdout and --out or --tee flags");
    }
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("{PROG}: error: {msg}");
    std::process::exit(1);
}
