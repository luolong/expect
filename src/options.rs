use std::path::PathBuf;
use clap::Parser;
use derive_getters::Getters;

#[derive(Debug, Getters, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
pub struct Options {

    /// read expected input from the file
    #[arg(short, long, value_name = "FILE")]
    from_file: Option<PathBuf>,

    /// Command line to start child process
    #[arg(trailing_var_arg = true, hide = true)]
    args: Vec<String>,
}
