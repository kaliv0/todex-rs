use clap::Parser;
use todexide::{cli::Cli, extractor::Extractor};

const PROG: &str = env!("CARGO_BIN_NAME");

fn exit_err(msg: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("{}: error: {msg}", PROG);
    std::process::exit(code);
}

fn main() {
    let cli = Cli::parse();

    if cli.ignore_default && cli.tokens.is_none() {
        exit_err("-i/--ignore-default flag requires -t/--tokens", 2);
    }

    let mut ext = match Extractor::new(
        &cli.path,
        cli.tokens,
        cli.ignore_default,
        cli.out.as_deref(),
    ) {
        Ok(ext) => ext,
        //??refactor with anyhow
        Err(e) => exit_err(e, 1),
    };

    if let Err(e) = ext.run() {
        exit_err(e, 1);
    }
}
