use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum Command {
    Patch,
    Makedeps,
    /// Print list of all interrupts described by an SVD file
    Interrupts {
        #[structopt(parse(from_os_str))]
        svd_file: PathBuf,

        /// Whether to print gaps in interrupt number sequence
        #[structopt(long)]
        no_gaps: bool,
    },
    Mmap,
}

#[derive(StructOpt, Debug)]
pub struct CliArgs {
    #[structopt(subcommand)]
    pub command: Command,
}

impl CliArgs {
    pub fn get_arguments() -> CliArgs {
        Self::from_args()
    }
}
