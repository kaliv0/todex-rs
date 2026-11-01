use anyhow::{Result, bail};
use fancy_regex::Regex;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::{exclude::ExcludeSpec, writer::Writer};

pub const DEFAULT_MAX_DEPTH: usize = 120;
pub const DEFAULT_OUT: &str = "TODO";
pub const DEFAULT_TOKENS: [&str; 2] = ["TODO", "FIXME"];
pub const DEFAULT_GLOBS: [&str; 2] = [".*", "__*"];

pub struct Extractor {
    root: PathBuf,
    token_pattern: Regex,
    pub exclude_spec: ExcludeSpec, //?? fix accessaiblity
    out: Writer,
    full_path: bool,
    concise: bool,
    recursive: bool,
    pub max_depth: usize, //?? fix accessaiblity
    debug: bool,
}

impl Extractor {
    pub fn new(
        path: impl AsRef<Path>,
        tokens: Option<Vec<String>>,
        ignore_default: bool,
        exclude: Option<Vec<String>>,
        include_hidden: bool,
        use_gitignore: bool,
        out: Option<&str>,
        full_path: bool,
        concise: bool,
        recursive: bool,
        max_depth: Option<usize>,
        stdout: bool,
        tee: bool,
        debug: bool,
    ) -> Result<Self> {
        let root = Self::prepare_root(path)?;
        let token_pattern =
            Self::prepare_token_pattern(tokens.unwrap_or_default(), ignore_default)?;
        let exclude_spec = Self::prepare_exclude_spec(
            &root,
            exclude.unwrap_or_default(),
            include_hidden,
            use_gitignore,
        )?;
        let max_depth = Self::prepare_max_depth(max_depth)?;
        let out = Self::prepare_out_writer(out.unwrap_or(DEFAULT_OUT), stdout, tee)?;

        Ok(Self {
            root,
            token_pattern,
            exclude_spec,
            out,
            full_path,
            concise,
            recursive,
            max_depth,
            debug,
        })
    }

    fn prepare_root(path: impl AsRef<Path>) -> Result<PathBuf> {
        let root = path.as_ref().to_path_buf();
        if root.is_symlink() {
            bail!("{} is a symlink", root.display());
        }
        if !root.exists() {
            bail!("{} not found", root.display());
        }
        Ok(root)
    }

    pub fn prepare_token_pattern(mut tokens: Vec<String>, ignore_default: bool) -> Result<Regex> {
        if !ignore_default {
            // prepend defaults to user-passed tokens
            //?? what if we use VecDeque to prepend?
            tokens.splice(0..0, DEFAULT_TOKENS.map(str::to_string));
        }
        if tokens.is_empty() {
            // prevented by CLI (-i without -t) but you can never be too careful
            bail!("no tokens to search for");
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

    fn prepare_out_writer(name: &str, stdout: bool, tee: bool) -> Result<Writer> {
        if stdout {
            return Writer::new(None, true);
        }

        let mut path = PathBuf::from(name);
        if path.is_dir() {
            path = path.join(DEFAULT_OUT);
        }
        Writer::new(Some(path), tee)
    }

    fn prepare_max_depth(max_depth: Option<usize>) -> Result<usize> {
        match max_depth {
            None => Ok(DEFAULT_MAX_DEPTH),
            Some(0) => bail!("max_depth must be a positive integer"),
            Some(n) => Ok(n),
        }
    }

    fn prepare_exclude_spec(
        root: &Path,
        mut exclude: Vec<String>,
        include_hidden: bool,
        use_gitignore: bool,
    ) -> Result<ExcludeSpec> {
        if use_gitignore && root.is_dir() {
            //?? what if we use VecDeque to prepend?
            exclude.splice(0..0, Self::get_gitignore_patterns(root)?);
        }

        if !include_hidden {
            exclude.splice(
                0..0,
                DEFAULT_GLOBS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            );
        }
        //??extract dedup() helper
        let mut seen = HashSet::new();
        exclude.retain(|p| seen.insert(p.clone()));
        ExcludeSpec::from_lines(&exclude)
    }

    fn get_gitignore_patterns(root: &Path) -> Result<Vec<String>> {
        let gitignore_file = root.join(".gitignore");
        if !gitignore_file.is_file() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(gitignore_file)?;
        Ok(text.lines().map(str::to_string).collect())
    }

    pub fn run(&mut self) -> Result<()> {
        if self.debug {
            self.out.debug("processing:\n")?;
        }

        let result = if self.root.is_file() {
            self.process_file(&self.root.clone()) //?? why clone
        } else if self.root.is_dir() {
            self.process_dir(&self.root.clone(), 0)
        } else {
            bail!("{} must be path to file or dir", self.root.display());
        };
        self.out.close()?;
        result
    }

    fn process_file(&mut self, path: &Path) -> Result<()> {
        if self.is_excluded(path, false) {
            return Ok(());
        }

        let display_name = self.prepare_path_name(path);
        if self.debug {
            self.out.debug(&format!("\t{display_name}\n"))?;
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        let mut lines_count = 0usize;
        let mut header_added = false;

        match (|| -> Result<()> {
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
                    &display_name,
                    &line,
                    lines_count,
                    matched.start(),
                    matched.end(),
                    header_added,
                )?;
            }
            Ok(())
        })() {
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|ioe| ioe.kind() == std::io::ErrorKind::InvalidData) =>
            {
                if self.debug {
                    self.out
                        .error(&format!("skipping {display_name}:\n{e}\n"))?;
                }
                Ok(())
            }
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
    ) -> Result<(usize, bool)> {
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
        line_num: usize,
    ) -> Result<usize> {
        let Some(body) = line[token_end..].strip_prefix('{') else {
            return Ok(1);
        };

        let err_msg = "invalid snippet count";
        let Some((body, _)) = body.split_once('}') else {
            bail!(
                "{}",
                self.add_debug_note(format!("{err_msg}: unclosed '{{'"), line, line_num)
            );
        };

        match body.parse::<usize>() {
            Ok(n) => Ok(n),
            Err(_) => bail!(
                "{}",
                self.add_debug_note(format!("{err_msg}: {body:?}"), line, line_num)
            ),
        }
    }

    fn add_debug_note(&self, msg: String, line: &str, line_num: usize) -> String {
        if self.debug {
            format!("{}\n\tline {}: {}", msg, line_num, line.trim())
        } else {
            msg
        }
    }

    fn prepare_path_name(&self, path: &Path) -> String {
        //??can we improve this
        if self.full_path {
            std::path::absolute(path)
                .unwrap_or_else(|_| path.to_path_buf())
                .display()
                .to_string()
        } else if self.recursive {
            path.display().to_string()
        } else {
            path.file_name()
                .map(|os_str| os_str.to_string_lossy().into_owned())
                .unwrap_or_default()
        }
    }

    fn process_dir(&mut self, path: &Path, depth: usize) -> Result<()> {
        if self.is_excluded(path, true) || depth > self.max_depth {
            return Ok(());
        }

        let mut entries = fs::read_dir(path)?
            .filter_map(|ent| ent.ok())
            .collect::<Vec<_>>();
        entries.sort_by_key(|ent| ent.file_name());

        let mut dirs = Vec::new();
        for entry in entries {
            // skip odd files
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            // metadata() doesn't follow symlinks
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

    fn is_excluded(&self, path: &Path, is_dir: bool) -> bool {
        // always process scan root
        if path == self.root {
            return false;
        }
        //skip summary file
        if self
            .out
            .path()
            .and_then(|out| out.canonicalize().ok())
            .is_some_and(|out| Some(out) == path.canonicalize().ok())
        {
            return true;
        }

        let Ok(rel) = path.strip_prefix(&self.root) else {
            return false;
        };
        self.exclude_spec.matches(&rel.to_string_lossy(), is_dir)
    }
}
