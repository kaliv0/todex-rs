//! Gitignore-style exclude matching for the walk.

use pathspec::GitIgnoreRegex;

use crate::error::Result;

/// Compiled exclude patterns for relative paths.
pub(crate) struct ExcludeSpec {
    patterns: Vec<GitIgnoreRegex>,
}

impl ExcludeSpec {
    /// Build from pattern lines. Inert lines are skipped.
    pub(crate) fn from_lines<S: AsRef<str>>(lines: &[S]) -> Result<Self> {
        Ok(Self {
            patterns: GitIgnoreRegex::from_spans_ignore_inert(lines)?,
        })
    }

    /// True if the relative path matches an exclude rule.
    pub(crate) fn matches(&self, rel: &str, is_dir: bool) -> bool {
        let outcome = if is_dir {
            pathspec::is_directory_match(&self.patterns, rel)
        } else {
            pathspec::is_file_path_match(&self.patterns, rel)
        };
        outcome == Some(true)
    }
}
