use clap::Parser;

const TRASH: &str = r#"
       ________________   ___/-\___     ___/-\___     ___/-\___
     / /             ||  |---------|   |---------|   |---------|
    / /              ||   |       |     | | | | |     |   |   |
   / /             __||   |       |     | | | | |     | | | | |
  / /   \\        I  ||   |       |     | | | | |     | | | | |
 (-------------------||   | | | | |     | | | | |     | | | | |
 ||               == ||   |_______|     |_______|     |_______|
 ||       TODEX      | =============================================
 ||          ____    |                                ____      |
( | o      / ____ \                                 / ____ \    |)
 ||      / / . . \ \                              / / . . \ \   |
[ |_____| | .   . | |____________________________| | .   . | |__]
          | .   . |                                | .   . |
           \_____/                                  \_____/

            Peak trash sniffer and TODO extractor
"#;

#[derive(Parser, Debug)]
#[command(
    name = "tdx",
    about = TRASH,
    version,
    help_template = "{about-with-newline}{all-args}{after-help}",
)]
pub struct Cli {
    #[arg(value_name = "PATH", help = "path to file or dir to process")]
    pub path: String,

    #[arg(
        short = 't',
        long = "tokens",
        num_args = 1..,
        value_name = "TOKEN",
        help = r#"list of tokens to search for (combined with default TODO, FIXME) e.g. WARN, REVISIT.
If the token is followed by {lines-count} e.g. #FIXME{3}
the extractor will include multiline snippet with the length specified between the curly braces:

    #FIXME{3} - revisit after release
    if self.foo == "bar":
        return f"fizz{buzz}"
"#
    )]
    pub tokens: Option<Vec<String>>,

    #[arg(
        short = 'i',
        long = "ignore-default",
        help = "use only -t tokens (skip default TODO, FIXME)"
    )]
    pub ignore_default: bool,

    #[arg(
        short = 'x',
        long = "exclude",
        num_args = 1..,
        value_name = "PATTERN",
        help = r#"git-like patterns to skip (paths, names, or globs - combined with default .* / __*), e.g.

    bar.py       any file named bar.py
    /bar.py      only top-level
    foo/bar.py   path under the scan root
    vendor/      directory and its contents
    **/*.pyc     nested matches
    !fizz.md     re-include after a broader exclude
"#
    )]
    pub exclude: Option<Vec<String>>,

    #[arg(
        short = 'H',
        long = "include-hidden",
        help = "do not apply default excludes for hidden names (.* / __*)"
    )]
    pub include_hidden: bool,

    #[arg(
        short = 'g',
        long = "use-gitignore",
        help = "use exclude patterns from .gitignore (if present in scan root) when traversing dirs"
    )]
    pub use_gitignore: bool,

    #[arg(
        short = 'f',
        long = "full-path",
        help = "display absolute dir/file path"
    )]
    pub full_path: bool,

    #[arg(
        short = 'c',
        long = "concise",
        help = "display only token messages e.g. '#TODO{5}: after release', ignore longer snippets"
    )]
    pub concise: bool,

    #[arg(
        short = 'r',
        long = "recursive",
        help = "traverse given dir path recursively"
    )]
    pub recursive: bool,

    #[arg(
        short = 'm',
        long = "max-depth",
        allow_hyphen_values = true,
        value_parser = require_positive_int,
        value_name = "N",
        help = "maximum depth of dir traversal - used with -r/--recursive flag (default: 120)"
    )]
    pub max_depth: Option<usize>,

    #[arg(short = 'd', long = "debug", help = "enable debug mode")]
    pub debug: bool,

    #[arg(
        short = 'o',
        long = "out",
        help = "path to output file - if existing dir is passed instead TODO out file will be saved inside the path"
    )]
    pub out: Option<String>,

    #[arg(short = 's', long = "stdout", help = "output result to stdout")]
    pub stdout: bool,

    #[arg(long = "tee", help = "output result to stdout and save to file")]
    pub tee: bool,
}

fn require_positive_int(s: &str) -> Result<usize, String> {
    //??why not std::io::Result
    // usize (u64) is too much for max_depth...
    match s.parse::<i64>() {
        Ok(num) if num > 0 => Ok(num as usize),
        Ok(num) => Err(format!("{num} must be positive")),
        Err(_) => Err(format!("{s} is not a valid number")),
    }
}
