use fancy_regex::Regex;
use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::writer::Writer;

pub const DEFAULT_OUT: &str = "TODO";
pub const DEFAULT_TOKENS: [&str; 2] = ["TODO", "FIXME"];

pub struct Extractor {
    root: PathBuf,
    token_pattern: Regex,
    out: Writer,
}

impl Extractor {
    pub fn new(
        path: impl AsRef<Path>,
        tokens: Option<Vec<String>>,
        ignore_default: bool,
        out: Option<&str>,
    ) -> std::io::Result<Self> {
        let root = Self::prepare_root(path)?;
        let token_pattern =
            Self::prepare_token_pattern(tokens.unwrap_or_default(), ignore_default)?;
        let out = Self::prepare_out_writer(out.unwrap_or(DEFAULT_OUT))?;

        Ok(Self {
            root,
            token_pattern,
            out,
        })
    }

    fn prepare_root(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let root = path.as_ref().to_path_buf();
        if !root.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} not found", root.display()),
            ));
        }
        Ok(root)
    }

    pub fn prepare_token_pattern(
        mut tokens: Vec<String>,
        ignore_default: bool,
    ) -> std::io::Result<Regex> {
        if !ignore_default {
            // prepend defaults to user-passed tokens
            tokens.splice(0..0, DEFAULT_TOKENS.map(str::to_string));
        }
        if tokens.is_empty() {
            // prevented by CLI (-i without -t) but you can never be too careful
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no tokens to search for",
            ));
        }

        // dedup() removes only consecutive duplicates
        // -> even after sorting it might be have same-length token sitting between copies
        let mut seen = HashSet::new();
        tokens.retain(|t| seen.insert(t.clone()));
        tokens.sort_by_key(|t| std::cmp::Reverse(t.len()));
        let alternations = tokens
            .iter()
            .map(|t| fancy_regex::escape(t))
            .collect::<Vec<_>>()
            .join("|");

        let pattern = format!(r"(?i)(?<!\w)(?:{alternations})(?!\w)");
        Ok(Regex::new(&pattern).expect("failed to compile token pattern"))
    }

    fn prepare_out_writer(name: &str) -> std::io::Result<Writer> {
        let mut path = PathBuf::from(name);
        if path.is_dir() {
            path = path.join(DEFAULT_OUT);
        }
        Writer::new(path)
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let result = self.process();
        self.out.close()?;
        result
    }

    fn process(&mut self) -> std::io::Result<()> {
        if self.root.is_file() {
            self.process_file(&self.root.clone()) //?? why clone
        } else if self.root.is_dir() {
            todo!()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} must be path to file or dir", self.root.display()),
            ))
        }
    }

    fn process_file(&mut self, path: &Path) -> std::io::Result<()> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        let mut lines_count = 0usize;
        let mut header_added = false;
        let display_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            lines_count += 1;

            if self
                .token_pattern
                .find(&line)
                .expect("token pattern find returned an error")
                .is_none()
            {
                continue;
            }

            if !header_added {
                self.out.write_header(display_name)?;
                header_added = true;
            }

            if !line.ends_with('\n') {
                line.push('\n'); // this is the last line of the file
            }
            self.out.write_entry(lines_count, &line)?;
        }
        Ok(())
    }
}
