use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use svdtools::{
    convert::convert_cli, interrupts::interrupts_cli, makedeps::makedeps_cli, mmap::mmap_cli,
    patch::patch_cli,
};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
enum Command {
    /// Patches an SVD file as specified by a YAML file
    Patch {
        #[clap(parse(from_os_str))]
        svd_file: PathBuf,
    },
    /// Generate Make dependency file listing dependencies for a YAML file.
    Makedeps {
        /// Input yaml file
        #[clap(parse(from_os_str))]
        yaml_file: PathBuf,

        /// Dependencies output file
        #[clap(parse(from_os_str))]
        deps_file: PathBuf,
    },
    /// Print list of all interrupts described by an SVD file
    Interrupts {
        #[clap(parse(from_os_str))]
        svd_file: PathBuf,

        /// Whether to print gaps in interrupt number sequence
        #[clap(long)]
        no_gaps: bool,
    },
    /// Generate text-based memory map of an SVD file.
    Mmap {
        #[clap(parse(from_os_str))]
        svd_file: PathBuf,
    },
    /// Convert SVD representation between file formats
    Convert {
        #[clap(parse(from_os_str))]
        in_path: PathBuf,
        #[clap(parse(from_os_str))]
        out_path: PathBuf,
        #[clap(long = "input-format")]
        input_format: Option<convert_cli::InputFormat>,
        #[clap(long = "output-format")]
        output_format: Option<convert_cli::OutputFormat>,
    },
}

impl Command {
    pub fn run(&self) -> Result<()> {
        match self {
            Self::Interrupts { svd_file, no_gaps } => {
                interrupts_cli::parse_device(svd_file, !no_gaps)?;
            }
            Self::Mmap { svd_file } => mmap_cli::parse_device(svd_file)?,
            Self::Patch { svd_file } => patch_cli::patch(svd_file)?,
            Self::Makedeps {
                yaml_file,
                deps_file,
            } => makedeps_cli::makedeps(yaml_file, deps_file)?,
            Self::Convert {
                in_path,
                out_path,
                input_format,
                output_format,
            } => convert_cli::convert(in_path, out_path, *input_format, *output_format)?,
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(subcommand)]
    command: Command,
}

pub fn run() {
    env_logger::init();

    let args = CliArgs::parse();
    if let Err(e) = args.command.run() {
        log::error!("{:?}", e);

        std::process::exit(1);
    }
}
