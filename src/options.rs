use std::path::PathBuf;

use clap::Parser;
use derive_getters::Getters;

use crate::matcher::Strategy;

#[derive(Debug, Getters, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
pub struct Options {
    /// read expected input from the file
    #[arg(short, long, value_name = "FILE")]
    from_file: Option<PathBuf>,

    /// Strategy used to match expected lines against actual input
    #[arg(short, long, value_enum, default_value = "exact")]
    strategy: Strategy,

    /// Add a token library file for `pattern` matching (repeatable)
    #[arg(short, long, value_name = "FILE")]
    patterns: Vec<PathBuf>,

    /// Define a one-off token for `pattern` matching, as NAME=REGEX (repeatable)
    #[arg(short = 'D', long, value_name = "NAME=REGEX")]
    define: Vec<String>,

    /// Override the extended patterns directory
    #[arg(long, value_name = "DIR")]
    patterns_dir: Option<PathBuf>,

    /// Command line to start child process
    #[arg(trailing_var_arg = true, hide = true)]
    args: Vec<String>,
}
