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

    let mut ext = match Extractor::new(
        &cli.path,
        cli.tokens,
        cli.ignore_default,
        cli.out.as_deref(),
        cli.concise,
        cli.recursive,
        cli.max_depth,
    ) {
        Ok(ext) => ext,
        //??refactor with anyhow
        Err(e) => exit_err(e, 1),
    };

    if let Err(e) = ext.run() {
        exit_err(e, 1);
    }
}
