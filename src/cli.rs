use std::path::PathBuf;
use structopt::StructOpt;

use crate::interrupt::interrupts;

#[derive(StructOpt, Debug)]
enum Command {
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

impl Command {
    pub fn run(&self) {
        match self {
            Self::Interrupts { svd_file, no_gaps } => {
                interrupts::parse_device(svd_file, !no_gaps);
            }
            _ => todo!(),
        };
    }
}

#[derive(StructOpt, Debug)]
struct CliArgs {
    #[structopt(subcommand)]
    command: Command,
}

pub fn run() {
    let args = CliArgs::from_args();
    args.command.run();
}
