use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use crate::ansi::strip;
use crate::library::Library;
use crate::matcher::{Matcher, Strategy};
use crate::options::Options;

mod ansi;
mod datetime;
mod library;
mod matcher;
mod options;
mod pattern;

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    let strategy = *options.strategy();
    let library = library(options.patterns_dir(), options.patterns(), options.define())?;

    if let Some(filename) = options.from_file() {
        let file = File::open(filename)
            .with_context(|| format!("Opening file: {}", filename.display()))?;

        let reader: &mut dyn BufRead = &mut BufReader::new(file);
        filter_matching_lines(reader, strategy, &library)
    } else {
        let args = options.args().join(" ");
        let expected_input = args.as_bytes();
        let reader: &mut dyn BufRead = &mut BufReader::new(expected_input);
        filter_matching_lines(reader, strategy, &library)
    }
}

/// Build the token library: the core set, then the extended directory, then any
/// per-invocation files and one-off definitions. Later layers override earlier
/// ones by name.
fn library(
    dir: &Option<PathBuf>,
    files: &[PathBuf],
    definitions: &[String],
) -> anyhow::Result<Library> {
    let mut library = Library::core()?;

    let extended = match dir {
        Some(dir) => Some(dir.clone()),
        None => Library::extended_dir(),
    };
    if let Some(dir) = extended {
        library.extend(Library::load_dir(&dir)?);
    }

    for path in files {
        library.extend(Library::load(path)?);
    }

    for definition in definitions {
        library.extend(Library::define(definition)?);
    }

    Ok(library)
}

fn filter_matching_lines(
    expected_input: &mut dyn BufRead,
    strategy: Strategy,
    library: &Library,
) -> anyhow::Result<()> {
    let mut out = stdout().lock();

    let mut expected_lines = expected_input
        .lines()
        .map_while(Result::ok)
        .map(|line| strategy.compile(&strip(&line), library))
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter();

    let mut expect_next: Option<Box<dyn Matcher>> = expected_lines.next();

    for actual_line in stdin().lines().map_while(Result::ok) {
        if let Some(expected_line) = &expect_next {
            if cfg!(debug_assertions) {
                eprintln!(
                    "Comparing: '{}' <=> '{actual_line}'",
                    expected_line.expected()
                )
            }

            if expected_line.matches(&strip(&actual_line)) {
                expect_next = expected_lines.next();
                continue;
            }
        }

        writeln!(out, "{actual_line}")?;
    }

    Ok(())
}
