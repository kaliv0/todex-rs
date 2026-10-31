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

    #[arg(short = 'c', long = "concise")]
    pub concise: bool,

    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,

    #[arg(short = 'm', long = "max-depth", allow_hyphen_values = true, value_parser = require_positive_int)]
    pub max_depth: Option<usize>,
}

fn require_positive_int(s: &str) -> Result<usize, String> {
    // usize (u64) is too much for max_depth...
    match s.parse::<i64>() {
        Ok(num) if num > 0 => Ok(num as usize),
        Ok(num) => Err(format!("{num} must be positive")),
        Err(_) => Err(format!("{s} is not a valid number")),
    }
}
