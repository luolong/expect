//! Pattern matching for expected lines.
//!
//! A pattern is a sequence of literal text and `<...>` tokens. The whole line
//! must match (the compiled regex is anchored at both ends).
//!
//! A `<name>` is a token only when the library recognizes `name`; any other
//! `<...>` (or a stray `<`) is literal text, so ordinary input containing angle
//! brackets can still be matched. Use `\<` to force a literal `<` before a token
//! name. `<_>` is the wildcard and is always available.
//!
//! A name is recognized, in order, as:
//!
//! 1. a `[tokens]` entry — a plain regex (this overrides everything below)
//! 2. `_` — the wildcard
//! 3. an alias, expanded from its spec (`ISODATETIME` is `<DATETIME:iso8601>`)
//! 4. a date/time kind — `DATE`, `TIME`, `DATETIME` (see `datetime.rs`)
//!
//! `DATE`, `TIME` and `DATETIME` take an optional `:parameter` narrowing the
//! match, e.g. `<DATETIME:iso8601>`, `<DATETIME:%Y-%m-%d>`, or
//! `<DATETIME:1 jan 2020 10:30:00.123UTC>`. Plain tokens do not take a
//! parameter yet.
//!
//! At a token's position the input may hold either a value of the token's class
//! or the placeholder text itself (`<IPV4>`, `<_>`, ...). The compiled regex
//! offers the literal placeholder as the first alternative, so the state machine
//! prefers the literal (cheaper, and what the user wrote) and falls back to the
//! value class only when the placeholder text is not present.

use regex::{escape, Regex};

use crate::datetime::{self, Kind};
use crate::library::Library;

/// Guard against alias cycles.
const MAX_DEPTH: usize = 16;

/// A compiled pattern.
#[derive(Debug)]
pub struct Pattern {
    regex: Regex,
}

impl Pattern {
    pub fn compile(source: &str, library: &Library) -> anyhow::Result<Self> {
        let body = fragment(source, library, 0)?;

        let regex = Regex::new(&format!("^(?:{body})$"))
            .map_err(|err| anyhow::anyhow!("invalid pattern '{source}': {err}"))?;

        Ok(Pattern { regex })
    }

    pub fn is_match(&self, line: &str) -> bool {
        self.regex.is_match(line)
    }
}

/// Build the regex fragment for `source`, without anchors.
fn fragment(source: &str, library: &Library, depth: usize) -> anyhow::Result<String> {
    let mut out = String::new();
    let mut rest = source;

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("\\<") {
            out.push_str(&escape("<"));
            rest = after;
            continue;
        }

        if rest.starts_with('<') {
            if let Some((name, param, placeholder, after)) = split_token(rest) {
                if let Some(value) = resolve(name, param, library, depth)? {
                    // Literal placeholder first, then the value class.
                    out.push_str(&format!("(?:{}|{})", escape(placeholder), value));
                    rest = after;
                    continue;
                }
            }
        }

        let c = rest.chars().next().unwrap();
        out.push_str(&escape(&c.to_string()));
        rest = &rest[c.len_utf8()..];
    }

    Ok(out)
}

/// Split `<name[:param]>` at the start of `s`, if it is a well-formed token.
fn split_token(s: &str) -> Option<(&str, Option<&str>, &str, &str)> {
    let close = s.find('>')?;
    let inner = &s[1..close];
    let placeholder = &s[..=close];
    let after = &s[close + 1..];

    let (name, param) = match inner.split_once(':') {
        Some((name, param)) => {
            let param = param.trim();
            (name.trim(), (!param.is_empty()).then_some(param))
        }
        None => (inner.trim(), None),
    };

    is_name(name).then_some((name, param, placeholder, after))
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();

    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// The value fragment for `name`, or `None` when `name` is not a token.
fn resolve(
    name: &str,
    param: Option<&str>,
    library: &Library,
    depth: usize,
) -> anyhow::Result<Option<String>> {
    if let Some(value) = library.token(name) {
        if param.is_some() {
            anyhow::bail!("token '{name}' does not take a parameter");
        }
        return Ok(Some(value.to_string()));
    }

    if name == "_" {
        return Ok(Some(".*?".to_string()));
    }

    if let Some(spec) = library.alias(name) {
        if param.is_some() {
            anyhow::bail!("token '{name}' does not take a parameter");
        }
        if depth >= MAX_DEPTH {
            anyhow::bail!("alias '{name}' expands too deeply (cycle?)");
        }
        return Ok(Some(fragment(spec, library, depth + 1)?));
    }

    if let Some(kind) = kind_of(name) {
        return Ok(Some(datetime_fragment(kind, param, library)?));
    }

    Ok(None)
}

fn kind_of(name: &str) -> Option<Kind> {
    match name.to_ascii_uppercase().as_str() {
        "DATE" => Some(Kind::Date),
        "TIME" => Some(Kind::Time),
        "DATETIME" => Some(Kind::DateTime),
        _ => None,
    }
}

/// The value fragment for a date/time token, honoring its parameter.
fn datetime_fragment(kind: Kind, param: Option<&str>, library: &Library) -> anyhow::Result<String> {
    let Some(param) = param else {
        return Ok(datetime::union(kind));
    };

    if param.contains('%') {
        return datetime::strftime_to_regex(param);
    }

    if let Some(fragment) = library.format(kind, param) {
        return Ok(fragment.to_string());
    }

    if let Some(fragment) = datetime::named_format(param) {
        return Ok(fragment.to_string());
    }

    datetime::infer_from_example(param)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::core().unwrap()
    }

    fn matches(pattern: &str, line: &str) -> bool {
        Pattern::compile(pattern, &library())
            .unwrap()
            .is_match(line)
    }

    #[test]
    fn literal_text_matches_exactly() {
        assert!(matches("hello world", "hello world"));
        assert!(!matches("hello world", "hello worlds"));
        assert!(!matches("hello world", "hello"));
    }

    #[test]
    fn metacharacters_are_literal() {
        assert!(matches("a.b", "a.b"));
        assert!(!matches("a.b", "axb"));
    }

    #[test]
    fn wildcard_matches_any_text() {
        assert!(matches("error: <_>", "error: disk full"));
        assert!(matches("<_>", ""));
    }

    #[test]
    fn wildcard_between_literals() {
        assert!(matches("from <_> to <_>", "from a to b"));
    }

    #[test]
    fn typed_token_matches_its_class() {
        assert!(matches("<IPV4>", "10.0.0.7"));
        assert!(!matches("<IPV4>", "not an ip"));
    }

    #[test]
    fn typed_token_also_matches_its_own_placeholder() {
        assert!(matches("<IPV4>", "<IPV4>"));
        assert!(matches("<_>", "<_>"));
    }

    #[test]
    fn unknown_token_is_literal() {
        assert!(matches("<NOPE>", "<NOPE>"));
        assert!(!matches("<NOPE>", "NOPE"));
    }

    #[test]
    fn escaped_angle_bracket_is_literal() {
        assert!(matches("\\<IPV4>", "<IPV4>"));
        assert!(!matches("\\<IPV4>", "10.0.0.7"));
    }

    #[test]
    fn datetime_bare_accepts_any_form() {
        assert!(matches("<DATETIME>", "2026-10-31T20:26:00Z"));
        assert!(matches("<DATETIME>", "2026-10-31"));
        assert!(matches("<DATETIME>", "20:26:00"));
        assert!(matches("<DATETIME>", "[10/Jun/2021:09:14:29 +0000]"));
        assert!(!matches("<DATETIME>", "hello"));
    }

    #[test]
    fn datetime_named_format_narrows() {
        assert!(matches("<DATETIME:iso8601>", "2026-10-31T20:26:00Z"));
        assert!(matches("<DATETIME:iso8601>", "2026-10-31 20:26:00"));
        assert!(!matches("<DATETIME:iso8601>", "31 Oct 2026 20:26:00 +0000"));
    }

    #[test]
    fn datetime_strftime_parameter() {
        assert!(matches("<DATETIME:%Y-%m-%d>", "2026-10-31"));
        assert!(!matches("<DATETIME:%Y-%m-%d>", "2026-10-31T20:26:00"));
    }

    #[test]
    fn datetime_example_parameter_infers_shape() {
        assert!(matches(
            "<DATETIME:1 jan 2020 10:30:00.123UTC>",
            "3 feb 2021 01:02:03.5UTC"
        ));
        assert!(!matches(
            "<DATETIME:1 jan 2020 10:30:00.123UTC>",
            "2026-10-31"
        ));
    }

    #[test]
    fn alias_expands_to_its_spec() {
        assert!(matches("<ISODATETIME>", "2026-10-31T20:26:00Z"));
        assert!(matches("<ISODATETIME>", "<ISODATETIME>"));
        assert!(!matches("<ISODATETIME>", "31 Oct 2026 20:26:00 +0000"));
        assert!(matches("<HTTPDATETIME>", "[10/Jun/2021:09:14:29 +0000]"));
    }

    #[test]
    fn token_definition_overrides_datetime_handling() {
        let mut library = library();
        library.extend(Library::parse("[tokens]\nDATETIME = 'x'\n").unwrap());

        let pattern = Pattern::compile("<DATETIME>", &library).unwrap();
        assert!(pattern.is_match("x"));
        assert!(!pattern.is_match("2026-10-31"));
    }

    #[test]
    fn token_definition_overrides_a_builtin_alias() {
        let mut library = library();
        library.extend(Library::parse("[tokens]\nISODATETIME = 'abc'\n").unwrap());

        let pattern = Pattern::compile("<ISODATETIME>", &library).unwrap();
        assert!(pattern.is_match("abc"));
        assert!(!pattern.is_match("2026-10-31"));
    }

    #[test]
    fn format_definition_overrides_a_builtin_format() {
        let mut library = library();
        library.extend(Library::parse("[formats.datetime]\niso8601 = 'X'\n").unwrap());

        let pattern = Pattern::compile("<DATETIME:iso8601>", &library).unwrap();
        assert!(pattern.is_match("X"));
        assert!(!pattern.is_match("2026-10-31T20:26:00Z"));
    }

    #[test]
    fn parameter_on_a_plain_token_is_reported() {
        assert!(Pattern::compile("<IPV4:127.0.0.0/16>", &library()).is_err());
    }

    #[test]
    fn bad_strftime_directive_is_reported() {
        assert!(Pattern::compile("<DATETIME:%Q>", &library()).is_err());
    }

    #[test]
    fn mixed_literals_and_tokens() {
        assert!(matches(
            "request took <NUM>ms from <IPV4>",
            "request took 12.5ms from 10.0.0.7"
        ));
        assert!(!matches(
            "request took <NUM>ms from <IPV4>",
            "request took 12.5ms from nowhere"
        ));
    }
}
