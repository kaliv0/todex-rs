use fancy_regex::Regex;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::writer::Writer;

pub const DEFAULT_MAX_DEPTH: usize = 120;
pub const DEFAULT_OUT: &str = "TODO";
pub const DEFAULT_TOKENS: [&str; 2] = ["TODO", "FIXME"];

pub struct Extractor {
    root: PathBuf,
    token_pattern: Regex,
    out: Writer,
    concise: bool,
    recursive: bool,
    pub max_depth: usize, //?? fix accessaiblity
}

impl Extractor {
    pub fn new(
        path: impl AsRef<Path>,
        tokens: Option<Vec<String>>,
        ignore_default: bool,
        out: Option<&str>,
        concise: bool,
        recursive: bool,
        max_depth: Option<usize>,
        stdout: bool,
        tee: bool,
    ) -> std::io::Result<Self> {
        let root = Self::prepare_root(path)?;
        let token_pattern =
            Self::prepare_token_pattern(tokens.unwrap_or_default(), ignore_default)?;
        let out = Self::prepare_out_writer(out.unwrap_or(DEFAULT_OUT), stdout, tee)?;
        let max_depth = Self::prepare_max_depth(max_depth)?;

        Ok(Self {
            root,
            token_pattern,
            out,
            concise,
            recursive,
            max_depth,
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

    fn prepare_out_writer(name: &str, stdout: bool, tee: bool) -> std::io::Result<Writer> {
        if stdout {
            return Writer::new(None, true);
        }

        let mut path = PathBuf::from(name);
        if path.is_dir() {
            path = path.join(DEFAULT_OUT);
        }
        Writer::new(Some(path), tee)
    }

    fn prepare_max_depth(max_depth: Option<usize>) -> std::io::Result<usize> {
        match max_depth {
            None => Ok(DEFAULT_MAX_DEPTH),
            Some(0) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "max_depth must be a positive integer",
            )),
            Some(n) => Ok(n),
        }
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let result = if self.root.is_file() {
            self.process_file(&self.root.clone()) //?? why clone
        } else if self.root.is_dir() {
            self.process_dir(&self.root.clone(), 0)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} must be path to file or dir", self.root.display()),
            ))
        };
        self.out.close()?;
        result
    }

    fn process_file(&mut self, path: &Path) -> std::io::Result<()> {
        // skip out_file
        if self
            .out
            .path()
            .and_then(|out| out.canonicalize().ok())
            .is_some_and(|out| Some(out) == path.canonicalize().ok())
        {
            return Ok(());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        let mut lines_count = 0usize;
        let mut header_added = false;
        let display_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        match (|| -> std::io::Result<()> {
            loop {
                line.clear();
                if reader.read_line(&mut line)? == 0 {
                    break;
                }
                lines_count += 1;

                let Some(matched) = self
                    .token_pattern
                    .find(&line)
                    .expect("token pattern find returned an error")
                else {
                    continue;
                };

                (lines_count, header_added) = self.write_hit(
                    &mut reader,
                    display_name,
                    &line,
                    lines_count,
                    matched.start(),
                    matched.end(),
                    header_added,
                )?;
            }
            Ok(())
        })() {
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Ok(()), // if debug - log and skip
            other => other,
        }
    }

    fn write_hit(
        &mut self,
        reader: &mut BufReader<File>,
        display_name: &str,
        line: &str,
        mut lines_count: usize,
        token_start: usize,
        token_end: usize,
        mut header_added: bool,
    ) -> std::io::Result<(usize, bool)> {
        if !header_added {
            self.out.write_header(display_name)?;
            header_added = true;
        }

        let snippet_count = self.get_snippet_count_after_token(line, token_end, lines_count)?;
        if self.concise {
            self.out.write_entry(lines_count, &line[token_start..])?;
            return Ok((lines_count, header_added));
        }

        let display_line_num = lines_count;
        let mut snippet = line.to_string();
        let mut line = String::new();
        for _ in 1..snippet_count {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            snippet.push_str(&line);
            lines_count += 1;
        }

        let entry = Self::format_snippet_entry(&snippet, display_line_num != lines_count);
        self.out.write_entry(display_line_num, &entry)?;
        Ok((lines_count, header_added))
    }

    fn format_snippet_entry(line: &str, multi: bool) -> String {
        let line = if line.ends_with('\n') {
            line.to_string() //?? unnecessary copy
        } else {
            format!("{line}\n") // this is the last line of the file
        };

        if multi {
            textwrap::indent(&textwrap::dedent(&format!("\n{line}")), "\t\t")
        } else {
            textwrap::dedent(&line)
        }
    }

    pub fn get_snippet_count_after_token(
        &self,
        line: &str,
        token_end: usize,
        _line_num: usize,
    ) -> std::io::Result<usize> {
        if !line[token_end..].starts_with('{') {
            return Ok(1);
        }

        // advance pointer to actual count
        let start = token_end + 1;
        let Some(end) = line[start..].find('}') else {
            //?? add debug note
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid snippet count: unclosed '{'",
            ));
        };

        let body = &line[start..start + end];
        body.parse::<usize>().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid snippet count: {body:?}"),
            )
        })
    }

    fn process_dir(&mut self, path: &Path, depth: usize) -> std::io::Result<()> {
        // skip if excluded
        if depth > self.max_depth {
            return Ok(());
        }

        let mut entries = fs::read_dir(path)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(|e| e.file_name());

        let mut dirs = Vec::new();
        for entry in entries {
            // skip odd files
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            // metadata() does not follow symlinks
            if meta.is_file() {
                self.process_file(&entry.path())?;
            } else if self.recursive && meta.is_dir() {
                dirs.push(entry.path());
            }
        }

        for dir in dirs {
            self.process_dir(&dir, depth + 1)?;
        }
        Ok(())
    }
}
