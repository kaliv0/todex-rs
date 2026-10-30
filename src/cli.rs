use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "tdx")]
pub struct Cli {
    pub path: String,

    #[arg(short = 't', long = "tokens", num_args = 1..)]
    pub tokens: Option<Vec<String>>,

    #[arg(short = 'i', long = "ignore-default")]
    pub ignore_default: bool,

    #[arg(short = 'o', long = "out")]
    pub out: Option<String>,
}
