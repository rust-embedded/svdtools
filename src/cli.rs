use std::path::PathBuf;
use structopt::StructOpt;

use crate::{interrupts::interrupts_cli, mmap::mmap_cli, patch::patch_cli};

#[derive(StructOpt, Debug)]
enum Command {
    Patch {
        #[structopt(parse(from_os_str))]
        svd_file: PathBuf,
    },

    Makedeps,
    /// Print list of all interrupts described by an SVD file
    Interrupts {
        #[structopt(parse(from_os_str))]
        svd_file: PathBuf,

        /// Whether to print gaps in interrupt number sequence
        #[structopt(long)]
        no_gaps: bool,
    },
    Mmap {
        #[structopt(parse(from_os_str))]
        svd_file: PathBuf,
    },
}

impl Command {
    pub fn run(&self) {
        match self {
            Self::Interrupts { svd_file, no_gaps } => {
                interrupts_cli::parse_device(svd_file, !no_gaps);
            }
            Self::Mmap { svd_file } => {
                mmap_cli::parse_device(svd_file);
            }
            Self::Patch { svd_file } => {
                patch_cli::patch(svd_file);
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
