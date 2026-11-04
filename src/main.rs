use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use clap::Parser;
use todex::{Cli, Extractor, ExtractorConfig, OutputDest, Result, Writer};

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
        ExtractorConfig {
            tokens: cli.tokens,
            ignore_default: cli.ignore_default,
            exclude: cli.exclude,
            include_hidden: cli.include_hidden,
            use_gitignore: cli.use_gitignore,
            full_path: cli.full_path,
            concise: cli.concise,
            recursive: cli.recursive,
            max_depth: cli.max_depth,
            debug: cli.debug,
        },
    )?;

    let dest = if cli.stdout {
        OutputDest::Stdout
    } else if cli.tee {
        OutputDest::tee(cli.out.as_deref())
    } else {
        OutputDest::file(cli.out.as_deref())
    };

    let mut writer = Writer::new(dest, interrupted)?;

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
