use regex::Regex;
use std::fmt::Display;
use std::string::ToString;

pub struct Matcher {
    line: String,
    pattern: Option<Regex>,
}

impl Display for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.line)
    }
}

impl From<String> for Matcher {
    fn from(s: String) -> Self {
        Matcher {
            line: s,
            pattern: None,
        }
    }
}

impl Matcher {
    pub fn of<T: ToString>(s: T) -> Matcher {
        Matcher {
            line: s.to_string(),
            pattern: None,
        }
    }
}

impl Matcher {
    pub fn matches<T: ToString>(&mut self, s: T) -> bool {
        let other = &s.to_string();
        if self.line.len() == other.len() {
            if &self.line == other {
                return true;
            }
        }

        if self.pattern.is_none() {
            // Lazily compile the pattern
            self.pattern = Regex::new(&self.line).ok();
        }

        if let Some(rx) = &self.pattern {
            rx.is_match_at(&other, 0)
        } else {
            false
        }
    }
}
