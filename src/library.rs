//! Named token libraries.
//!
//! A library maps token names (e.g. `IPV4`) to the regex fragment that matches a
//! value of that type. `Pattern` expands `<NAME>` tokens through a library.
//!
//! Three layers, later overriding earlier by name, with no collision error:
//!
//! 1. the core set, embedded in the binary (`patterns/*.toml`)
//! 2. the extended set, `*.toml` files in `$XDG_CONFIG_HOME/expect/patterns`
//! 3. one-off definitions from the command line (`--define NAME=REGEX`)
//!
//! A file may hold four tables:
//!
//! * `[tokens]` — plain name-to-regex entries
//! * `[formats.date]`, `[formats.time]`, `[formats.datetime]` — named date and
//!   time formats, used by `<DATETIME:name>` and friends
//! * `[aliases]` — names defined as specs, e.g. `ISODATETIME = "<DATETIME:iso8601>"`
//!
//! `DATE`, `TIME`, `DATETIME` and their built-in aliases have special handling
//! unless overridden by a `[tokens]` entry of the same name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::datetime::Kind;

/// The core set, embedded in the binary.
const CORE: &[(&str, &str)] = &[
    ("numbers.toml", include_str!("../patterns/numbers.toml")),
    (
        "identifiers.toml",
        include_str!("../patterns/identifiers.toml"),
    ),
    ("network.toml", include_str!("../patterns/network.toml")),
    ("text.toml", include_str!("../patterns/text.toml")),
];

/// Built-in aliases, defined as specs and overridable like any other name.
const BUILTIN_ALIASES: &[(&str, &str)] = &[
    ("ISODATETIME", "<DATETIME:iso8601>"),
    ("RFC3339", "<DATETIME:rfc3339>"),
    ("HTTPDATETIME", "<DATETIME:http>"),
];

/// A set of named token fragments, formats, and aliases.
#[derive(Debug, Default, Clone)]
pub struct Library {
    tokens: HashMap<String, String>,
    formats: HashMap<(Kind, String), String>,
    aliases: HashMap<String, String>,
}

impl Library {
    /// The core set embedded in the binary, plus the built-in aliases.
    pub fn core() -> anyhow::Result<Self> {
        let mut library = Library::default();

        for (name, contents) in CORE {
            library.extend(Self::parse(contents).map_err(|err| anyhow::anyhow!("{name}: {err}"))?);
        }

        for (name, spec) in BUILTIN_ALIASES {
            library
                .aliases
                .insert((*name).to_string(), (*spec).to_string());
        }

        Ok(library)
    }

    /// Read every `*.toml` file in `dir`, merged in name order.
    ///
    /// A missing directory is not an error: it simply yields an empty library.
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Library::default()),
            Err(err) => {
                return Err(anyhow::anyhow!("reading {}: {err}", dir.display()));
            }
        };

        let mut files = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                files.push(path);
            }
        }
        files.sort();

        let mut library = Library::default();
        for path in files {
            library.extend(Self::load(&path)?);
        }

        Ok(library)
    }

    /// Read a library from a single TOML file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("reading {}: {err}", path.display()))?;

        Self::parse(&text).map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))
    }

    /// The extended patterns directory, if a config directory can be determined.
    pub fn extended_dir() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

        Some(base.join("expect").join("patterns"))
    }

    /// A plain token fragment, matched case-insensitively.
    pub fn token(&self, name: &str) -> Option<&str> {
        self.tokens
            .get(&name.to_ascii_uppercase())
            .map(String::as_str)
    }

    /// An alias specification, matched case-insensitively.
    pub fn alias(&self, name: &str) -> Option<&str> {
        self.aliases
            .get(&name.to_ascii_uppercase())
            .map(String::as_str)
    }

    /// A named format of `kind`, matched case-insensitively.
    pub fn format(&self, kind: Kind, name: &str) -> Option<&str> {
        self.formats
            .get(&(kind, name.to_ascii_lowercase()))
            .map(String::as_str)
    }

    /// Add `other`'s entries, overriding any of the same name.
    pub fn extend(&mut self, other: Library) {
        self.tokens.extend(other.tokens);
        self.formats.extend(other.formats);
        self.aliases.extend(other.aliases);
    }

    /// Define a single token from a `NAME=REGEX` command-line definition.
    pub fn define(definition: &str) -> anyhow::Result<Self> {
        let (name, fragment) = definition.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("invalid definition '{definition}': expected NAME=REGEX")
        })?;

        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow::anyhow!(
                "invalid definition '{definition}': empty name"
            ));
        }

        Self::parse(&format!(
            "[tokens]\n{name} = {}\n",
            toml_string(fragment.trim())
        ))
    }

    /// Parse a library from TOML text. Every fragment must be a valid regex.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let table: toml::Table = text
            .parse()
            .map_err(|err| anyhow::anyhow!("invalid TOML: {err}"))?;

        let mut library = Library::default();

        if let Some(section) = table.get("tokens") {
            let section = section
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("'tokens' must be a table"))?;

            for (name, value) in section {
                let fragment = value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("token '{name}' must be a string"))?;
                validate(fragment).map_err(|err| anyhow::anyhow!("token '{name}': {err}"))?;
                library
                    .tokens
                    .insert(name.to_ascii_uppercase(), fragment.to_string());
            }
        }

        if let Some(section) = table.get("formats") {
            let section = section
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("'formats' must be a table"))?;

            for (key, value) in section {
                let kind = Kind::from_section(key).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown format kind 'formats.{key}' (expected date, time, or datetime)"
                    )
                })?;

                let entries = value
                    .as_table()
                    .ok_or_else(|| anyhow::anyhow!("'formats.{key}' must be a table"))?;

                for (name, value) in entries {
                    let fragment = value
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("format '{name}' must be a string"))?;
                    validate(fragment).map_err(|err| anyhow::anyhow!("format '{name}': {err}"))?;
                    library
                        .formats
                        .insert((kind, name.to_ascii_lowercase()), fragment.to_string());
                }
            }
        }

        if let Some(section) = table.get("aliases") {
            let section = section
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("'aliases' must be a table"))?;

            for (name, value) in section {
                let spec = value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("alias '{name}' must be a string"))?;
                library
                    .aliases
                    .insert(name.to_ascii_uppercase(), spec.to_string());
            }
        }

        Ok(library)
    }
}

fn validate(fragment: &str) -> anyhow::Result<()> {
    Regex::new(fragment)
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("invalid regex: {err}"))
}

/// Quote `value` as a TOML basic string, so a fragment with quotes or
/// backslashes round-trips through the parser unchanged.
fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tokens_from_toml() {
        let library = Library::parse("[tokens]\nGREETING = 'hello|hi'\n").unwrap();
        assert_eq!(library.token("GREETING"), Some("hello|hi"));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let library = Library::parse("[tokens]\nIPV4 = 'x'\n").unwrap();
        assert_eq!(library.token("ipv4"), Some("x"));
    }

    #[test]
    fn unknown_name_has_no_fragment() {
        let library = Library::parse("").unwrap();
        assert!(library.token("NOPE").is_none());
    }

    #[test]
    fn extend_overrides_by_name() {
        let mut library = Library::parse("[tokens]\nINT = '[0-9]+'\nIPV4 = 'x'\n").unwrap();
        library.extend(Library::parse("[tokens]\nINT = '[0-9]'\n").unwrap());

        assert_eq!(library.token("INT"), Some("[0-9]"));
        // Untouched tokens survive.
        assert_eq!(library.token("IPV4"), Some("x"));
    }

    #[test]
    fn parses_named_formats_by_kind() {
        let library = Library::parse(
            "[formats.datetime]\nmine = '\\d{4}-\\d{2}-\\d{2}'\n[formats.date]\nmine = '\\d{4}'\n",
        )
        .unwrap();

        assert_eq!(
            library.format(Kind::DateTime, "mine"),
            Some(r"\d{4}-\d{2}-\d{2}")
        );
        assert_eq!(library.format(Kind::Date, "mine"), Some(r"\d{4}"));
        assert!(library.format(Kind::Time, "mine").is_none());
    }

    #[test]
    fn parses_aliases() {
        let library = Library::parse("[aliases]\nMYDT = '<DATETIME:iso8601>'\n").unwrap();
        assert_eq!(library.alias("mydt"), Some("<DATETIME:iso8601>"));
    }

    #[test]
    fn unknown_format_kind_is_reported() {
        assert!(Library::parse("[formats.nope]\nx = 'y'\n").is_err());
    }

    #[test]
    fn invalid_fragment_is_reported() {
        assert!(Library::parse("[tokens]\nBAD = '('\n").is_err());
    }

    #[test]
    fn non_string_fragment_is_reported() {
        assert!(Library::parse("[tokens]\nBAD = 1\n").is_err());
    }

    #[test]
    fn empty_library_is_valid() {
        assert!(Library::parse("").is_ok());
    }

    #[test]
    fn missing_directory_yields_empty_library() {
        let library = Library::load_dir(Path::new("/nonexistent/expect/patterns")).unwrap();
        assert!(library.token("IPV4").is_none());
    }

    #[test]
    fn define_parses_name_and_fragment() {
        let library = Library::define("TRACEID=[0-9a-f]{16}").unwrap();
        assert_eq!(library.token("TRACEID"), Some("[0-9a-f]{16}"));
    }

    #[test]
    fn define_keeps_backslashes_and_quotes() {
        let library = Library::define(r#"QS="\d+"x"#).unwrap();
        assert_eq!(library.token("QS"), Some(r#""\d+"x"#));
    }

    #[test]
    fn define_without_equals_is_reported() {
        assert!(Library::define("TRACEID").is_err());
    }

    #[test]
    fn define_with_empty_name_is_reported() {
        assert!(Library::define("=x").is_err());
    }

    #[test]
    fn define_with_invalid_regex_is_reported() {
        assert!(Library::define("BAD=(").is_err());
    }

    #[test]
    fn core_set_defines_the_documented_tokens() {
        let library = Library::core().unwrap();

        for name in [
            "INT", "NUM", "HEX", "PORT", "UUID", "MD5", "SHA1", "SHA256", "IPV4", "IPV6", "MAC",
            "HOSTNAME", "EMAIL", "URL", "WORD",
        ] {
            assert!(library.token(name).is_some(), "missing token: {name}");
        }
    }

    #[test]
    fn core_set_defines_the_builtin_aliases() {
        let library = Library::core().unwrap();
        assert_eq!(library.alias("ISODATETIME"), Some("<DATETIME:iso8601>"));
        assert_eq!(library.alias("HTTPDATETIME"), Some("<DATETIME:http>"));
    }

    #[test]
    fn core_set_fragments_compile() {
        // Parsing validates every fragment; a bad one fails the load.
        assert!(Library::core().is_ok());
    }
}
