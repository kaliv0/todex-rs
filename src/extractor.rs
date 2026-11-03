use fancy_regex::Regex;
use std::{
    borrow::Cow,
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf, absolute},
};

use crate::{
    error::{Error, Result},
    exclude::ExcludeSpec,
    writer::Writer,
};

pub const DEFAULT_MAX_DEPTH: usize = 120;
pub const DEFAULT_TOKENS: [&str; 2] = ["TODO", "FIXME"];
pub const DEFAULT_GLOBS: [&str; 2] = [".*", "__*"];

pub struct Extractor {
    root: PathBuf,
    token_pattern: Regex,
    exclude_spec: ExcludeSpec,
    full_path: bool,
    concise: bool,
    recursive: bool,
    max_depth: usize,
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
        full_path: bool,
        concise: bool,
        recursive: bool,
        max_depth: Option<usize>,
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

        Ok(Self {
            root,
            token_pattern,
            exclude_spec,
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
            return Err(Error::Symlink(root));
        }
        if !root.exists() {
            return Err(Error::NotFound(root));
        }
        Ok(root)
    }

    fn prepare_token_pattern(mut tokens: Vec<String>, ignore_default: bool) -> Result<Regex> {
        if !ignore_default {
            // prepend defaults to user-passed tokens
            //?? what if we use VecDeque to prepend?
            tokens.splice(0..0, DEFAULT_TOKENS.map(str::to_string));
        }
        if tokens.is_empty() {
            // prevented by CLI (-i without -t) but you can never be too careful
            return Err(Error::NoTokens);
        }

        Self::dedup(&mut tokens);
        tokens.sort_unstable_by_key(|t| std::cmp::Reverse(t.len()));
        let alternations = tokens
            .iter()
            .map(|t| fancy_regex::escape(t))
            .collect::<Vec<_>>()
            .join("|");

        let pattern = format!(r"(?i)(?<!\w)(?:{alternations})(?!\w)");
        Ok(Regex::new(&pattern).expect("failed to compile token pattern"))
    }

    fn prepare_max_depth(max_depth: Option<usize>) -> Result<usize> {
        match max_depth {
            None => Ok(DEFAULT_MAX_DEPTH),
            Some(0) => Err(Error::InvalidMaxDepth),
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
        Self::dedup(&mut exclude);
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

    pub fn run(&self, writer: &mut Writer) -> Result<()> {
        let walk = (|| -> Result<()> {
            if self.debug {
                writer.debug("processing:\n")?;
            }

            if self.root.is_file() {
                self.process_file(&self.root, writer)
            } else if self.root.is_dir() {
                self.process_dir(&self.root, 0, writer)
            } else {
                Err(Error::NotFileOrDir(self.root.clone()))
            }
        })();

        writer.close()?; // always flush out_file
        walk
    }

    fn process_file(&self, path: &Path, writer: &mut Writer) -> Result<()> {
        if writer.interrupted() {
            return Err(Error::Interrupted);
        }
        if self.is_excluded(path, false, writer) {
            return Ok(());
        }

        let display_name = self.prepare_path_name(path);
        if self.debug {
            writer.debug(&format!("\t{display_name}\n"))?;
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
                    writer,
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
            Err(e) if e.is_invalid_data() => {
                if self.debug {
                    writer.error(&format!("skipping {display_name}:\n{e}\n"))?;
                }
                Ok(())
            }
            other => other,
        }
    }

    fn write_hit(
        &self,
        writer: &mut Writer,
        reader: &mut BufReader<File>,
        display_name: &str,
        line: &str,
        mut lines_count: usize,
        token_start: usize,
        token_end: usize,
        mut header_added: bool,
    ) -> Result<(usize, bool)> {
        if !header_added {
            writer.write_header(display_name)?;
            header_added = true;
        }

        let snippet_count = self.get_snippet_count_after_token(line, token_end, lines_count)?;
        if self.concise {
            writer.write_entry(lines_count, &line[token_start..])?;
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
        writer.write_entry(display_line_num, &entry)?;
        Ok((lines_count, header_added))
    }

    fn format_snippet_entry(line: &str, multi: bool) -> String {
        let line: Cow<'_, str> = if line.ends_with('\n') {
            Cow::Borrowed(line)
        } else {
            Cow::Owned(format!("{line}\n")) // this is the last line of the file
        };

        if multi {
            textwrap::indent(&textwrap::dedent(&format!("\n{line}")), "\t\t")
        } else {
            textwrap::dedent(&line)
        }
    }

    fn get_snippet_count_after_token(
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
            return Err(Error::InvalidSnippet(self.add_debug_note(
                format!("{err_msg}: unclosed '{{'"),
                line,
                line_num,
            )));
        };

        match body.parse::<usize>() {
            Ok(n) => Ok(n),
            Err(_) => Err(Error::InvalidSnippet(self.add_debug_note(
                format!("{err_msg}: {body:?}"),
                line,
                line_num,
            ))),
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
        if self.full_path {
            absolute(path)
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

    fn process_dir(&self, path: &Path, depth: usize, writer: &mut Writer) -> Result<()> {
        if writer.interrupted() {
            return Err(Error::Interrupted);
        }
        if self.is_excluded(path, true, writer) || depth > self.max_depth {
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
                self.process_file(&entry.path(), writer)?;
            } else if self.recursive && meta.is_dir() {
                dirs.push(entry.path());
            }
        }

        for dir in dirs {
            self.process_dir(&dir, depth + 1, writer)?;
        }
        Ok(())
    }

    fn is_excluded(&self, path: &Path, is_dir: bool, writer: &Writer) -> bool {
        // always process scan root
        if path == self.root {
            return false;
        }
        //skip summary file
        if writer
            .path()
            .and_then(|p| absolute(p).ok())
            .is_some_and(|p| Some(p) == absolute(path).ok())
        {
            return true;
        }

        let Ok(rel) = path.strip_prefix(&self.root) else {
            return false;
        };
        self.exclude_spec.matches(&rel.to_string_lossy(), is_dir)
    }

    fn dedup(items: &mut Vec<String>) {
        let mut seen = HashSet::new();
        items.retain(|i| seen.insert(i.clone()));
    }
}

////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn matched(tokens: &[&str], ignore_default: bool, line: &str) -> Option<String> {
        let tokens = tokens.iter().map(|t| (*t).to_string()).collect();
        Extractor::prepare_token_pattern(tokens, ignore_default)
            .expect("prepare_token_pattern")
            .find(line)
            .expect("token pattern find returned an error")
            .map(|m| m.as_str().to_string())
    }

    #[test]
    fn empty_with_ignore_default_raises() {
        let err = Extractor::prepare_token_pattern(Vec::new(), true).unwrap_err();
        assert!(err.to_string().contains("no tokens"));
    }

    #[test]
    fn prepare_token_pattern_cases() {
        let cases: &[(&[&str], bool, &str, Option<&str>)] = &[
            (&[], false, "# TODO: x", Some("TODO")),
            (&[], false, "# FIXME: x", Some("FIXME")),
            (&["WARN"], false, "# WARN: x", Some("WARN")),
            (&["WARN"], false, "# TODO: x", Some("TODO")),
            (&["WARN"], true, "# WARN: x", Some("WARN")),
            (&["WARN"], true, "# TODO: x", None),
            (&["WARN"], true, "# FIXME: x", None),
            (&["WARN", "REVISIT"], true, "WARN here", Some("WARN")),
            (&["WARN", "REVISIT"], true, "REVISIT later", Some("REVISIT")),
            (&["TODO", "TODO", "FIXME"], false, "TODO", Some("TODO")),
            (&["TODO", "TODO", "FIXME"], false, "FIXME", Some("FIXME")),
        ];
        for (tokens, ignore_default, line, expected) in cases {
            assert_eq!(
                matched(tokens, *ignore_default, line).as_deref(),
                *expected,
                "tokens={tokens:?} ignore_default={ignore_default} line={line:?}"
            );
        }
    }

    #[test]
    fn word_boundaries_and_case() {
        let cases = [
            ("todo: lower", Some("todo")),
            ("Todo: mixed", Some("Todo")),
            ("fIxMe: weird", Some("fIxMe")),
            ("TODO", Some("TODO")),
            ("#TODO: x", Some("TODO")),
            ("//FIXME x", Some("FIXME")),
            ("(TODO)", Some("TODO")),
            ("[FIXME]", Some("FIXME")),
            ("please TODO this soon", Some("TODO")),
            ("TODOS remaining", None),
            ("MYTODO done", None),
            ("XXTODOYY", None),
        ];
        for (line, expected) in cases {
            assert_eq!(
                matched(&[], false, line).as_deref(),
                expected,
                "line={line:?}"
            );
        }
    }

    #[test]
    fn prefers_longer_token() {
        assert_eq!(
            matched(&["FIX", "FIXME"], true, "FIXME: later").as_deref(),
            Some("FIXME")
        );
        assert_eq!(
            matched(&["FIX", "FIXME"], true, "FIX: now").as_deref(),
            Some("FIX")
        );
    }

    #[test]
    fn overlap_leftmost_longest() {
        assert_eq!(
            matched(&["AB", "ABC", "BC"], true, " ABC ").as_deref(),
            Some("ABC")
        );
    }

    #[test]
    fn non_ascii_token_boundary() {
        assert_eq!(
            matched(&["中文标记"], true, "中文标记: alone").as_deref(),
            Some("中文标记")
        );
        assert_eq!(
            matched(&["中文标记"], true, "前中文标记后").as_deref(),
            None
        );
    }

    #[test]
    fn special_characters_are_literal() {
        let cases: &[(&[&str], &str, Option<&str>)] = &[
            (&["C++"], "use C++ here", Some("C++")),
            (&["file.txt"], "see file.txt now", Some("file.txt")),
            (&["file.txt"], "fileXtxt", None),
            (&["a.b*"], "a.b* value", Some("a.b*")),
            (&["A+"], "A+", Some("A+")),
            (&["A+"], "AAA", None),
            (&["@FIXME"], "# @FIXME: x", Some("@FIXME")),
            (&["@FIXME"], "# FIXME: bare", None),
            (
                &["Do we need this!"],
                "# Do we need this!",
                Some("Do we need this!"),
            ),
            (&["!!!URGENT!!!"], "# !!!URGENT!!!", Some("!!!URGENT!!!")),
        ];
        for (tokens, line, expected) in cases {
            assert_eq!(
                matched(tokens, true, line).as_deref(),
                *expected,
                "tokens={tokens:?} line={line:?}"
            );
        }
    }

    #[test]
    fn get_snippet_count_after_token() {
        let dir = tempfile::tempdir().unwrap();
        let ext = Extractor::new(
            dir.path(),
            None,
            false,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap();
        for (line, expected) in [("# TODO: x", 1usize), ("# TODO{3} x", 3)] {
            let token_end = line.find("TODO").unwrap() + "TODO".len();
            assert_eq!(
                ext.get_snippet_count_after_token(line, token_end, 1)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn max_depth_none_uses_default() {
        let dir = tempfile::tempdir().unwrap();
        let ext = Extractor::new(
            dir.path(),
            None,
            false,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap();
        assert_eq!(ext.max_depth, DEFAULT_MAX_DEPTH);
    }

    #[test]
    fn exclude_spec_defaults_bang_and_comments() {
        let dir = tempfile::tempdir().unwrap();
        let ext = Extractor::new(
            dir.path(),
            None,
            false,
            Some(vec![
                "vendor/".into(),
                "!vendor/keep.py".into(),
                "# comment".into(),
                "".into(),
                "  ".into(),
            ]),
            false,
            false,
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap();
        let spec = &ext.exclude_spec;

        assert!(spec.matches(".hidden.py", false));
        assert!(spec.matches("__init__.py", false));
        assert!(spec.matches("vendor", true) || spec.matches("vendor/", true));
        assert!(spec.matches("vendor/lib.py", false));
        assert!(!spec.matches("vendor/keep.py", false));
        assert!(!spec.matches("app.py", false));
    }

    #[test]
    fn file_path_g_does_not_load_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.py");
        fs::write(&file, "# TODO: x\n").unwrap();
        let gitignore = dir.path().join(".gitignore");
        fs::write(&gitignore, "vendor/\n").unwrap();
        let ext = Extractor::new(
            &file, None, false, None, false, true, false, false, false, None, false,
        )
        .unwrap();
        assert!(!ext.exclude_spec.matches("vendor/lib.py", false));
    }
}
