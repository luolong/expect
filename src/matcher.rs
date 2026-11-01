use regex::Regex;

use crate::library::Library;
use crate::pattern::Pattern;

/// Decides whether an actual input line corresponds to an expected line.
///
/// Strategies differ only in how strictly they compare. The strategy is
/// selected on the command line, not inferred from the input.
pub trait Matcher {
    /// The expected line this matcher was built from, for diagnostics.
    fn expected(&self) -> &str;

    /// Does `line` match this expected line?
    fn matches(&self, line: &str) -> bool;
}

/// Matching strategy applied to every expected line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Strategy {
    /// Byte-for-byte equality.
    Exact,
    /// The expected line is a regular expression matched anywhere in the line.
    Regex,
    /// The expected line is a pattern: literals plus `<...>` tokens, e.g. `<_>`, `<IPV4>`.
    Pattern,
}

impl Strategy {
    pub fn compile(self, expected: &str, library: &Library) -> anyhow::Result<Box<dyn Matcher>> {
        match self {
            Strategy::Exact => Ok(Box::new(Exact::new(expected))),
            Strategy::Regex => Ok(Box::new(RegexMatcher::new(expected)?)),
            Strategy::Pattern => Ok(Box::new(PatternMatcher::new(expected, library)?)),
        }
    }
}

struct Exact {
    expected: String,
}

impl Exact {
    fn new(expected: &str) -> Self {
        Exact {
            expected: expected.to_string(),
        }
    }
}

impl Matcher for Exact {
    fn expected(&self) -> &str {
        &self.expected
    }

    fn matches(&self, line: &str) -> bool {
        self.expected == line
    }
}

struct RegexMatcher {
    expected: String,
    pattern: Regex,
}

impl RegexMatcher {
    fn new(expected: &str) -> anyhow::Result<Self> {
        let pattern = Regex::new(expected)
            .map_err(|err| anyhow::anyhow!("invalid pattern '{expected}': {err}"))?;

        Ok(RegexMatcher {
            expected: expected.to_string(),
            pattern,
        })
    }
}

impl Matcher for RegexMatcher {
    fn expected(&self) -> &str {
        &self.expected
    }

    fn matches(&self, line: &str) -> bool {
        self.pattern.is_match(line)
    }
}

struct PatternMatcher {
    expected: String,
    pattern: Pattern,
}

impl PatternMatcher {
    fn new(expected: &str, library: &Library) -> anyhow::Result<Self> {
        Ok(PatternMatcher {
            expected: expected.to_string(),
            pattern: Pattern::compile(expected, library)?,
        })
    }
}

impl Matcher for PatternMatcher {
    fn expected(&self) -> &str {
        &self.expected
    }

    fn matches(&self, line: &str) -> bool {
        self.pattern.is_match(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::core().unwrap()
    }

    #[test]
    fn exact_matches_only_identical_lines() {
        let matcher = Strategy::Exact.compile("error", &library()).unwrap();
        assert!(matcher.matches("error"));
        assert!(!matcher.matches("errors"));
        assert!(!matcher.matches("an error occurred"));
        assert!(!matcher.matches("Error"));
    }

    #[test]
    fn exact_treats_metacharacters_literally() {
        let matcher = Strategy::Exact.compile("a.b", &library()).unwrap();
        assert!(matcher.matches("a.b"));
        assert!(!matcher.matches("axb"));
    }

    #[test]
    fn regex_matches_anywhere_in_the_line() {
        let matcher = Strategy::Regex.compile("wor.d", &library()).unwrap();
        assert!(matcher.matches("hello world"));
        assert!(!matcher.matches("hello there"));
    }

    #[test]
    fn regex_is_anchored_when_the_pattern_is() {
        let matcher = Strategy::Regex.compile("^warning:", &library()).unwrap();
        assert!(matcher.matches("warning: unused import"));
        assert!(!matcher.matches("  warning: indented"));
    }

    #[test]
    fn invalid_regex_is_reported() {
        assert!(Strategy::Regex.compile("(", &library()).is_err());
    }

    #[test]
    fn invalid_regex_is_fine_when_exact() {
        assert!(Strategy::Exact.compile("(", &library()).is_ok());
    }

    #[test]
    fn pattern_strategy_matches_typed_segments() {
        let matcher = Strategy::Pattern
            .compile("<_> took <NUM>ms from <IPV4>", &library())
            .unwrap();
        assert!(matcher.matches("request took 12.5ms from 10.0.0.7"));
        assert!(!matcher.matches("request took 12.5ms from nowhere"));
    }

    #[test]
    fn pattern_strategy_treats_unknown_token_as_literal() {
        let matcher = Strategy::Pattern.compile("<NOPE>", &library()).unwrap();
        assert!(matcher.matches("<NOPE>"));
        assert!(!matcher.matches("NOPE"));
    }
}
