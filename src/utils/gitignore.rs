use std::path::Path;

/// Lightweight .gitignore pattern matcher.
///
/// Supports: `*`, `**`, `?`, comments (`#`), negation (`!`), dir-only trailing `/`,
/// anchored patterns (containing `/`). Only reads a single .gitignore file
/// (no nested .gitignore support).
#[derive(Default)]
pub struct GitIgnore {
    rules: Vec<IgnoreRule>,
}

struct IgnoreRule {
    pattern: String,
    negated: bool,
    dir_only: bool,
    anchored: bool,
}

impl GitIgnore {
    /// Parse a .gitignore file. Returns empty ruleset if the file doesn't exist or is unreadable.
    pub fn from_file(path: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        let rules = content.lines().filter_map(parse_line).collect();
        Self { rules }
    }

    /// Append extra exclusion patterns (same syntax as .gitignore lines).
    pub fn extend_patterns(&mut self, patterns: &[String]) {
        for line in patterns {
            if let Some(rule) = parse_line(line) {
                self.rules.push(rule);
            }
        }
    }

    /// Check whether `relative_path` should be ignored.
    /// `is_dir` must be true when the path refers to a directory (affects trailing-`/` rules).
    #[must_use]
    pub fn is_ignored(&self, relative_path: &str, is_dir: bool) -> bool {
        let mut ignored = false;

        for rule in &self.rules {
            if rule.dir_only && !is_dir {
                continue;
            }

            let matches = if rule.anchored {
                glob_match(&rule.pattern, relative_path)
            } else {
                // Non-anchored: match against the last path component
                let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
                glob_match(&rule.pattern, name)
            };

            if matches {
                ignored = !rule.negated;
            }
        }

        ignored
    }
}

/// Parse a single .gitignore line into an `IgnoreRule`.
///
/// Works entirely on `&str` slices to avoid intermediate allocations,
/// creating a single owned `String` only for the final pattern.
fn parse_line(line: &str) -> Option<IgnoreRule> {
    let mut s = line.trim_end();
    if s.is_empty() || s.starts_with('#') {
        return None;
    }

    // Negation
    let negated = s.starts_with('!');
    if negated {
        s = &s[1..];
    }

    // Dir-only trailing slash
    let dir_only = s.ends_with('/');
    if dir_only {
        s = &s[..s.len() - 1];
    }

    // Anchored: leading `/` or contains `/` in the middle
    let anchored;
    if let Some(rest) = s.strip_prefix('/') {
        anchored = true;
        s = rest;
    } else {
        anchored = s.contains('/');
    }

    if s.is_empty() {
        return None;
    }

    Some(IgnoreRule {
        pattern: s.to_string(),
        negated,
        dir_only,
        anchored,
    })
}

/// Match a gitignore-style glob pattern against text.
///
/// - `*` matches any sequence of characters except `/`
/// - `**` matches any sequence of characters including `/`
/// - `?` matches any single character except `/`
fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_bytes(p: &[u8], t: &[u8]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        // ** — matches everything including /
        (Some(b'*'), _) if p.starts_with(b"**") => {
            let rest = p[2..].strip_prefix(b"/").unwrap_or(&p[2..]);
            glob_match_bytes(rest, t) || (!t.is_empty() && glob_match_bytes(p, &t[1..]))
        }

        // * — matches any sequence except /
        (Some(b'*'), _) => {
            glob_match_bytes(&p[1..], t)
                || (!t.is_empty() && t[0] != b'/' && glob_match_bytes(p, &t[1..]))
        }

        // ? — matches single char except /
        (Some(b'?'), Some(&c)) if c != b'/' => glob_match_bytes(&p[1..], &t[1..]),

        // Literal match
        (Some(&pc), Some(&tc)) if pc == tc => glob_match_bytes(&p[1..], &t[1..]),

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── glob_match ──────────────────────────────────────────────────

    #[test]
    fn literal_match() {
        assert!(glob_match("foo", "foo"));
        assert!(!glob_match("foo", "bar"));
        assert!(!glob_match("foo", "foobar"));
        assert!(!glob_match("foobar", "foo"));
    }

    #[test]
    fn star_matches_non_slash() {
        assert!(glob_match("*.pyc", "foo.pyc"));
        assert!(glob_match("*.pyc", ".pyc"));
        assert!(!glob_match("*.pyc", "dir/foo.pyc"));
        assert!(glob_match("foo*", "foobar"));
        assert!(glob_match("f*o", "foo"));
        assert!(glob_match("f*o", "fo"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("*", "a/b"));
    }

    #[test]
    fn double_star_matches_everything() {
        assert!(glob_match("**", "anything"));
        assert!(glob_match("**", "a/b/c"));
        assert!(glob_match("**/foo", "foo"));
        assert!(glob_match("**/foo", "a/foo"));
        assert!(glob_match("**/foo", "a/b/foo"));
        assert!(!glob_match("**/foo", "a/b/foobar"));
    }

    #[test]
    fn double_star_mid_pattern() {
        assert!(glob_match("a/**/b", "a/b"));
        assert!(glob_match("a/**/b", "a/x/b"));
        assert!(glob_match("a/**/b", "a/x/y/b"));
        assert!(!glob_match("a/**/b", "a/x/y/c"));
    }

    #[test]
    fn double_star_trailing() {
        assert!(glob_match("foo/**", "foo/bar"));
        assert!(glob_match("foo/**", "foo/bar/baz"));
        assert!(!glob_match("foo/**", "foo"));
    }

    #[test]
    fn question_mark() {
        assert!(glob_match("fo?", "foo"));
        assert!(glob_match("fo?", "fox"));
        assert!(!glob_match("fo?", "fo"));
        assert!(!glob_match("fo?", "fo/"));
    }

    #[test]
    fn trailing_stars_match_empty() {
        assert!(glob_match("foo*", "foo"));
        assert!(glob_match("foo**", "foo"));
    }

    // ── parse_line ──────────────────────────────────────────────────

    #[test]
    fn parse_skips_comments_and_blanks() {
        assert!(parse_line("").is_none());
        assert!(parse_line("  ").is_none());
        assert!(parse_line("# comment").is_none());
    }

    #[test]
    fn parse_simple_pattern() {
        let rule = parse_line("node_modules").unwrap();
        assert_eq!(rule.pattern, "node_modules");
        assert!(!rule.negated);
        assert!(!rule.dir_only);
        assert!(!rule.anchored);
    }

    #[test]
    fn parse_negated() {
        let rule = parse_line("!important.log").unwrap();
        assert_eq!(rule.pattern, "important.log");
        assert!(rule.negated);
    }

    #[test]
    fn parse_dir_only() {
        let rule = parse_line("build/").unwrap();
        assert_eq!(rule.pattern, "build");
        assert!(rule.dir_only);
        assert!(!rule.anchored);
    }

    #[test]
    fn parse_anchored_leading_slash() {
        let rule = parse_line("/build").unwrap();
        assert_eq!(rule.pattern, "build");
        assert!(rule.anchored);
    }

    #[test]
    fn parse_anchored_contains_slash() {
        let rule = parse_line("src/generated").unwrap();
        assert_eq!(rule.pattern, "src/generated");
        assert!(rule.anchored);
    }

    // ── is_ignored ──────────────────────────────────────────────────

    #[test]
    fn simple_name_matches_anywhere() {
        let gi = GitIgnore {
            rules: vec![parse_line("node_modules").unwrap()],
        };
        assert!(gi.is_ignored("node_modules", true));
        assert!(gi.is_ignored("a/node_modules", true));
        assert!(gi.is_ignored("a/b/node_modules", true));
    }

    #[test]
    fn extension_pattern_matches_any_level() {
        let gi = GitIgnore {
            rules: vec![parse_line("*.pyc").unwrap()],
        };
        assert!(gi.is_ignored("foo.pyc", false));
        assert!(gi.is_ignored("a/b/foo.pyc", false));
        assert!(!gi.is_ignored("foo.py", false));
    }

    #[test]
    fn anchored_pattern_root_only() {
        let gi = GitIgnore {
            rules: vec![parse_line("/build").unwrap()],
        };
        assert!(gi.is_ignored("build", true));
        assert!(!gi.is_ignored("a/build", true));
    }

    #[test]
    fn dir_only_skips_files() {
        let gi = GitIgnore {
            rules: vec![parse_line("build/").unwrap()],
        };
        assert!(gi.is_ignored("build", true));
        assert!(!gi.is_ignored("build", false));
    }

    #[test]
    fn negation_overrides() {
        let gi = GitIgnore {
            rules: vec![
                parse_line("*.log").unwrap(),
                parse_line("!important.log").unwrap(),
            ],
        };
        assert!(gi.is_ignored("debug.log", false));
        assert!(!gi.is_ignored("important.log", false));
    }

    #[test]
    fn double_star_in_gitignore() {
        let gi = GitIgnore {
            rules: vec![parse_line("**/logs").unwrap()],
        };
        assert!(gi.is_ignored("logs", true));
        assert!(gi.is_ignored("a/logs", true));
        assert!(gi.is_ignored("a/b/logs", true));
    }

    #[test]
    fn anchored_path_pattern() {
        let gi = GitIgnore {
            rules: vec![parse_line("src/generated").unwrap()],
        };
        assert!(gi.is_ignored("src/generated", true));
        assert!(!gi.is_ignored("other/src/generated", true));
    }

    #[test]
    fn from_file_missing() {
        let gi = GitIgnore::from_file(Path::new("/nonexistent/.gitignore"));
        assert!(!gi.is_ignored("anything", false));
    }

    #[test]
    fn extend_patterns_adds_rules() {
        let mut gi = GitIgnore::default();
        gi.extend_patterns(&["*.log".to_string(), "tmp/".to_string()]);
        assert!(gi.is_ignored("debug.log", false));
        assert!(gi.is_ignored("tmp", true));
        assert!(!gi.is_ignored("tmp", false));
    }
}
