use clap::Parser;
use todex::{cli::Cli, extractor::Extractor};

const PROG: &str = env!("CARGO_BIN_NAME");

fn exit_err(msg: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("{}: error: {msg}", PROG);
    std::process::exit(code);
}

fn main() {
    let cli = Cli::parse();

    // validate dependent flags
    if cli.ignore_default && cli.tokens.is_none() {
        exit_err("-i/--ignore-default flag requires -t/--tokens", 2);
    }
    if cli.max_depth.is_some() && !cli.recursive {
        exit_err("-m/--max-depth flag requires -r/--recursive", 2);
    }
    if cli.stdout && (cli.out.is_some() || cli.tee) {
        exit_err("shouldn't mix --stdout and --out or --tee flags", 2);
    }

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
    )
    .unwrap_or_else(|e| exit_err(e, 1));

    if let Err(e) = extractor.run() {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(141)
        }
        exit_err(e, 1);
    }
}
