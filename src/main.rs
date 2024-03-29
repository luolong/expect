use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, Write};

use anyhow::Context;
use clap::Parser;
use matcher::Matcher;

use crate::options::Options;

mod matcher;
mod options;

fn main() -> anyhow::Result<()> {
    let options = Options::parse();

    if let Some(filename) = options.from_file() {
        let file = File::open(filename)
            .with_context(|| format!("Opening file: {}", filename.display()))?;

        let reader: &mut dyn BufRead = &mut BufReader::new(file);
        filter_matching_lines(reader)
    } else {
        let args = options.args().join(" ");
        let expected_input = args.as_bytes();
        let reader: &mut dyn BufRead = &mut BufReader::new(expected_input);
        filter_matching_lines(reader)
    }
}

fn filter_matching_lines(expected_input: &mut dyn BufRead) -> anyhow::Result<()> {
    let mut out = stdout().lock();

    let mut expected_lines = expected_input
        .lines()
        .map_while(Result::ok)
        .map(Matcher::from);

    let mut expect_next = expected_lines.next();

    for actual_line in stdin().lines().map_while(Result::ok) {
        if let Some(expected_line) = &mut expect_next {
            if cfg!(debug_assertions) {
                eprintln!("Comparing: '{expected_line}' <=> '{actual_line}'")
            }

            if expected_line.matches(&actual_line) {
                expect_next = expected_lines.next();
                continue;
            }
        }

        writeln!(out, "{actual_line}")?;
    }

    Ok(())
}
