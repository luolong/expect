//! Date and time formats.
//!
//! `DATE`, `TIME` and `DATETIME` are related: the first two are the parts of the
//! third. Each is matched as the union of the known formats of its kind, so
//! `<DATETIME>` accepts any valid date, time, or date-and-time.
//!
//! `DATETIME` (and `DATE` / `TIME`) take an optional parameter narrowing the
//! match, resolved in this order:
//!
//! 1. a `strftime`-style specification, recognized by containing `%`
//!    (`<DATETIME:%Y-%m-%dT%H:%M:%S%:z>`)
//! 2. a named format, from the library's `[formats]` table or the built-ins
//!    below (`<DATETIME:iso8601>`)
//! 3. anything else is an example, from which a shape is inferred
//!    (`<DATETIME:1 jan 2020 10:30:00.123UTC>`)

use regex::Regex;

/// Whether a format is a date, a time, or a combination of both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Date,
    Time,
    DateTime,
}

impl Kind {
    /// The kind named by a `[formats.*]` section key.
    pub fn from_section(section: &str) -> Option<Self> {
        match section {
            "date" => Some(Kind::Date),
            "time" => Some(Kind::Time),
            "datetime" => Some(Kind::DateTime),
            _ => None,
        }
    }

    /// The kinds whose formats `self` accepts. `DATETIME` accepts the parts.
    pub fn parts(self) -> &'static [Kind] {
        match self {
            Kind::Date => &[Kind::Date],
            Kind::Time => &[Kind::Time],
            Kind::DateTime => &[Kind::DateTime, Kind::Date, Kind::Time],
        }
    }
}

/// The built-in named formats.
const FORMATS: &[(&str, Kind, &str)] = &[
    ("iso8601", Kind::DateTime, ISO_DATETIME),
    ("rfc3339", Kind::DateTime, ISO_DATETIME),
    ("iso8601_date", Kind::Date, ISO_DATE),
    ("iso8601_time", Kind::Time, ISO_TIME),
    ("http", Kind::DateTime, HTTP),
    ("syslog", Kind::DateTime, SYSLOG),
    ("rfc2822", Kind::DateTime, RFC2822),
    ("system", Kind::DateTime, SYSTEM),
];

const ISO_DATETIME: &str =
    r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?";
const ISO_DATE: &str = r"\d{4}-\d{2}-\d{2}";
const ISO_TIME: &str = r"\d{2}:\d{2}:\d{2}(?:\.\d+)?";
const HTTP: &str = r"\[\d{2}/[A-Za-z]{3}/\d{4}:\d{2}:\d{2}:\d{2} [+-]\d{4}\]";
const SYSLOG: &str = r"[A-Z][a-z]{2} {1,2}\d{1,2} \d{2}:\d{2}:\d{2}";
const RFC2822: &str = r"(?:[A-Za-z]{3},? )?\d{1,2} [A-Za-z]{3} \d{4} \d{2}:\d{2}:\d{2} [+-]\d{4}";
const SYSTEM: &str = r"[A-Za-z]{3} [A-Za-z]{3} {1,2}\d{1,2} \d{2}:\d{2}:\d{2} [A-Za-z]{2,5} \d{4}";
const SLASH_DATE: &str = r"\d{1,2}[-/.]\d{1,2}[-/.]\d{2,4}";
const CLOCK_TIME: &str = r"\d{1,2}:\d{2}(?::\d{2})?(?: ?[AaPp]\.?[Mm]\.?)?";

/// A built-in named format, matched case-insensitively.
pub fn named_format(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    FORMATS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, fragment)| *fragment)
}

/// The union of the known formats accepted by `kind`, as a regex fragment.
///
/// `DATETIME` accepts the date and time kinds too, since they are its parts.
pub fn union(kind: Kind) -> String {
    let mut fragments: Vec<&str> = FORMATS
        .iter()
        .filter(|(_, k, _)| kind.parts().contains(k))
        .map(|(_, _, fragment)| *fragment)
        .collect();

    for part in kind.parts() {
        match part {
            Kind::Date => fragments.push(SLASH_DATE),
            Kind::Time => fragments.push(CLOCK_TIME),
            Kind::DateTime => {}
        }
    }

    fragments
        .iter()
        .map(|fragment| format!("(?:{fragment})"))
        .collect::<Vec<_>>()
        .join("|")
}

/// Compile a `strftime`-style specification to a regex fragment.
pub fn strftime_to_regex(spec: &str) -> anyhow::Result<String> {
    let mut out = String::new();
    let mut chars = spec.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push_str(&regex::escape(&c.to_string()));
            continue;
        }

        // Skip an optional `:` / `E` / `O` modifier, e.g. `%:z`.
        while let Some(&c) = chars.peek() {
            if c == ':' || c == 'E' || c == 'O' {
                chars.next();
            } else {
                break;
            }
        }

        let fragment = match chars.next() {
            Some('Y') => r"\d{4}",
            Some('y') | Some('C') => r"\d{2}",
            Some('m') => r"(?:0[1-9]|1[0-2])",
            Some('d') => r"(?:0[1-9]|[12]\d|3[01])",
            Some('e') => r"(?:[ 1-9]|[12]\d|3[01])",
            Some('H') => r"(?:[01]\d|2[0-3])",
            Some('I') => r"(?:0[1-9]|1[0-2])",
            Some('M') => r"[0-5]\d",
            Some('S') => r"(?:[0-5]\d|60)",
            Some('f') => r"\d{1,9}",
            Some('L') => r"\d{3}",
            Some('j') => r"\d{3}",
            Some('b') | Some('h') => r"[A-Za-z]{3}",
            Some('B') => r"[A-Za-z]+",
            Some('a') => r"[A-Za-z]{3}",
            Some('A') => r"[A-Za-z]+",
            Some('p') => r"[AaPp][Mm]",
            Some('z') => r"[+-]\d{2}:?\d{2}",
            Some('Z') => r"[A-Za-z]{1,6}(?:[+-]\d{2}:?\d{2})?",
            Some('s') => r"\d+",
            Some('n') => r"\n",
            Some('t') => r"\t",
            Some('%') => "%",
            Some('F') => r"\d{4}-\d{2}-\d{2}",
            Some('T') => r"\d{2}:\d{2}:\d{2}",
            Some('R') => r"\d{2}:\d{2}",
            Some('D') => r"\d{2}/\d{2}/\d{2}",
            Some('r') => r"\d{2}:\d{2}:\d{2} [AaPp][Mm]",
            Some(other) => anyhow::bail!("unsupported strftime directive '%{other}'"),
            None => anyhow::bail!("trailing '%' in specification"),
        };

        out.push_str(fragment);
    }

    Ok(out)
}

/// Infer a regex fragment from an example date, generalizing recognizable date
/// and time components and keeping everything else literal.
pub fn infer_from_example(example: &str) -> anyhow::Result<String> {
    // Tried in order at the cursor; the first anchored match wins, so the more
    // specific atoms must come first.
    let atoms: Vec<(Regex, &str)> = [
        (
            r"^\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d+)?",
            r"\d{1,2}:\d{2}:\d{2}(?:\.\d+)?",
        ),
        (r"^\d{4}-\d{2}-\d{2}", r"\d{4}-\d{2}-\d{2}"),
        (
            r"^\d{1,2}[-/.]\d{1,2}[-/.]\d{2,4}",
            r"\d{1,2}[-/.]\d{1,2}[-/.]\d{2,4}",
        ),
        (r"^[+-]\d{2}:?\d{2}", r"[+-]\d{2}:?\d{2}"),
        (
            r"(?i)^(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)",
            r"[A-Za-z]{3}",
        ),
        (r"(?i)^(?:mon|tue|wed|thu|fri|sat|sun)", r"[A-Za-z]{3}"),
        (r"(?i)^[ap]\.?m\.?", r"[AaPp]\.?[Mm]\.?"),
        // Timezone names generalize; other words stay literal.
        (r"^(?:UTC|GMT|[A-Z]{2,5})", r"[A-Za-z]{2,5}"),
        (r"^\d{4}", r"\d{4}"),
        (r"^\d+", r"\d+"),
    ]
    .into_iter()
    .map(|(pattern, fragment)| (Regex::new(pattern).unwrap(), fragment))
    .collect();

    let mut out = String::new();
    let mut pos = 0;

    while pos < example.len() {
        let rest = &example[pos..];

        match atoms
            .iter()
            .find_map(|(regex, fragment)| regex.find(rest).map(|m| (m.end(), *fragment)))
        {
            Some((end, fragment)) => {
                out.push_str(fragment);
                pos += end;
            }
            None => {
                let c = rest.chars().next().unwrap();
                out.push_str(&regex::escape(&c.to_string()));
                pos += c.len_utf8();
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchored(fragment: &str) -> Regex {
        Regex::new(&format!("^(?:{fragment})$")).unwrap()
    }

    #[test]
    fn strftime_iso_like_specification() {
        let regex = anchored(&strftime_to_regex("%Y-%m-%dT%H:%M:%S%:z").unwrap());
        assert!(regex.is_match("2026-10-31T20:26:00+00:00"));
        assert!(!regex.is_match("2026-10-31 20:26:00"));
    }

    #[test]
    fn strftime_rejects_unknown_directive() {
        assert!(strftime_to_regex("%Q").is_err());
    }

    #[test]
    fn strftime_rejects_trailing_percent() {
        assert!(strftime_to_regex("abc%").is_err());
    }

    #[test]
    fn example_infers_a_shape() {
        let regex = anchored(&infer_from_example("1 jan 2020 10:30:00.123UTC").unwrap());

        assert!(regex.is_match("1 jan 2020 10:30:00.123UTC"));
        assert!(regex.is_match("23 Dec 1999 23:59:59.5GMT"));
        assert!(!regex.is_match("not a date at all"));
    }

    #[test]
    fn example_infers_iso_shape() {
        let regex = anchored(&infer_from_example("2026-10-31T20:26:00").unwrap());
        assert!(regex.is_match("2026-10-31T20:26:00"));
        assert!(regex.is_match("1999-01-02T03:04:05"));
        assert!(!regex.is_match("2026-10-31"));
    }

    #[test]
    fn example_keeps_literals_literal() {
        let regex = anchored(&infer_from_example("on 2020-01-01").unwrap());
        assert!(regex.is_match("on 2020-01-01"));
        assert!(!regex.is_match("at 2020-01-01"));
    }

    #[test]
    fn datetime_union_accepts_every_kind() {
        let regex = anchored(&union(Kind::DateTime));
        assert!(regex.is_match("2026-10-31")); // date only
        assert!(regex.is_match("20:26:00")); // time only
        assert!(regex.is_match("2026-10-31T20:26:00Z")); // date and time
        assert!(regex.is_match("31 Oct 2026 20:26:00 +0000"));
        assert!(regex.is_match("[10/Jun/2021:09:14:29 +0000]"));
        assert!(!regex.is_match("hello world"));
    }

    #[test]
    fn date_union_excludes_times() {
        let regex = anchored(&union(Kind::Date));
        assert!(regex.is_match("2026-10-31"));
        assert!(!regex.is_match("20:26:00"));
    }

    #[test]
    fn time_union_excludes_dates() {
        let regex = anchored(&union(Kind::Time));
        assert!(regex.is_match("20:26:00"));
        assert!(!regex.is_match("2026-10-31"));
    }

    #[test]
    fn named_formats_are_case_insensitive() {
        assert_eq!(named_format("iso8601"), named_format("ISO8601"));
        assert!(named_format("http").is_some());
        assert!(named_format("nope").is_none());
    }

    #[test]
    fn sections_map_to_kinds() {
        assert_eq!(Kind::from_section("date"), Some(Kind::Date));
        assert_eq!(Kind::from_section("datetime"), Some(Kind::DateTime));
        assert_eq!(Kind::from_section("nope"), None);
    }
}
