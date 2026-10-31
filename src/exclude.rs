use pathspec::GitIgnoreRegex;

pub struct ExcludeSpec {
    patterns: Vec<GitIgnoreRegex>,
}

impl ExcludeSpec {
    pub fn from_lines<S: AsRef<str>>(lines: &[S]) -> std::io::Result<Self> {
        Ok(Self {
            patterns: GitIgnoreRegex::from_spans_ignore_inert(lines).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
            })?,
        })
    }

    pub fn matches(&self, rel: &str, is_dir: bool) -> bool {
        let outcome = if is_dir {
            pathspec::is_directory_match(&self.patterns, rel)
        } else {
            pathspec::is_file_path_match(&self.patterns, rel)
        };

        outcome == Some(true)
    }
}
