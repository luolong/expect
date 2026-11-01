//! Stream alignment of expected lines against actual input.
//!
//! The expected lines form an ordered queue. An actual line is suppressed when
//! it matches an expected line at or after the cursor, and that expected line
//! (and any before it) is then consumed. Matching a line *later* in the queue
//! means the expected lines in between never appeared, so they are skipped.
//!
//! This stays stream alignment: expected lines are consumed in order and at
//! most once each. It is not a denylist. A line is suppressed only by
//! consuming an expected line, so repeated input cannot be swallowed by a
//! single expectation, and unexpected lines are never eaten.

use crate::matcher::Matcher;

/// Consumes expected lines as actual lines arrive.
pub struct Aligner {
    expected: Vec<Box<dyn Matcher>>,
    cursor: usize,
}

impl Aligner {
    pub fn new(expected: Vec<Box<dyn Matcher>>) -> Self {
        Aligner {
            expected,
            cursor: 0,
        }
    }

    /// Whether `line` is expected, consuming the matching expected line.
    ///
    /// Returns `true` when the line should be suppressed.
    pub fn consume(&mut self, line: &str) -> bool {
        match (self.cursor..self.expected.len()).find(|&i| self.expected[i].matches(line)) {
            Some(index) => {
                self.cursor = index + 1;
                true
            }
            None => false,
        }
    }

    /// The next expected line, for diagnostics.
    pub fn current(&self) -> Option<&str> {
        self.expected.get(self.cursor).map(|m| m.expected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use crate::matcher::Strategy;

    fn aligner(expected: &[&str]) -> Aligner {
        let library = Library::core().unwrap();
        let matchers = expected
            .iter()
            .map(|line| Strategy::Exact.compile(line, &library).unwrap())
            .collect();
        Aligner::new(matchers)
    }

    /// The lines an aligner would print from `actual`.
    fn printed(expected: &[&str], actual: &[&str]) -> Vec<String> {
        let mut aligner = aligner(expected);
        actual
            .iter()
            .filter(|line| !aligner.consume(line))
            .map(|line| line.to_string())
            .collect()
    }

    #[test]
    fn in_order_input_is_fully_suppressed() {
        assert!(printed(&["a", "b", "c"], &["a", "b", "c"]).is_empty());
    }

    #[test]
    fn unexpected_lines_are_printed() {
        assert_eq!(printed(&["a", "b"], &["a", "x", "b"]), ["x"]);
    }

    #[test]
    fn missing_expected_line_does_not_wedge_the_cursor() {
        // The defect: `b` never appears, but `c` is still expected.
        assert!(printed(&["a", "b", "c"], &["a", "c"]).is_empty());
    }

    #[test]
    fn several_missing_expected_lines_are_skipped() {
        assert!(printed(&["a", "b", "c", "d"], &["a", "d"]).is_empty());
    }

    #[test]
    fn repeated_input_is_not_swallowed_by_one_expectation() {
        // A denylist would eat both `a`; alignment consumes the expectation once.
        assert_eq!(printed(&["a"], &["a", "a"]), ["a"]);
    }

    #[test]
    fn each_expectation_suppresses_at_most_one_line() {
        assert_eq!(printed(&["a", "b"], &["a", "a", "b"]), ["a"]);
    }

    #[test]
    fn exhausted_expectations_print_the_rest() {
        assert_eq!(printed(&["a"], &["a", "b", "c"]), ["b", "c"]);
    }

    #[test]
    fn empty_expectations_print_everything() {
        assert_eq!(printed(&[], &["a", "b"]), ["a", "b"]);
    }

    #[test]
    fn empty_input_prints_nothing() {
        assert!(printed(&["a", "b"], &[]).is_empty());
    }

    #[test]
    fn leading_unexpected_lines_are_printed() {
        assert_eq!(printed(&["b", "c"], &["x", "b", "c"]), ["x"]);
    }

    #[test]
    fn out_of_order_input_prints_the_displaced_line() {
        // `b` arrives before the expected `a`, so `b`'s expectation is consumed
        // first and `a` is left to match later. Only one line is printed; the
        // alignment never eats both.
        let result = printed(&["a", "b"], &["b", "a"]);
        assert_eq!(result.len(), 1);
    }
}
