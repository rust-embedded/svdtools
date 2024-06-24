use anyhow::{Ok, Result};
use clap::Parser;
use std::{fs::File, io::Write, path::PathBuf, str::FromStr};

use svdtools::{
    convert::convert_cli,
    html::html_cli,
    html::htmlcompare_cli,
    info,
    interrupts::interrupts_cli,
    makedeps::makedeps_cli,
    mmap::mmap_cli,
    patch::{patch_cli, EnumAutoDerive},
};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
enum Command {
    /// Patches an SVD file as specified by a YAML file
    Patch {
        /// Path to input YAML file
        yaml_file: PathBuf,

        /// Path to output file. By default it just adds `.patched` at the end
        out_path: Option<PathBuf>,

        /// Path to format config file
        ///
        /// If not specified, the default format config will be used.
        #[clap(long)]
        format_config: Option<PathBuf>,

        /// Check for errors after patching
        #[clap(long)]
        post_validate: bool,

        /// When a patch error happens print formatted yaml with all rules included
        #[clap(long)]
        show_patch_on_error: bool,

        /// Derive level when several identical enumerationValues added in a field
        #[clap(long)]
        enum_derive: Option<EnumAutoDerive>,
    },
    ExpandPatch {
        /// Path to input YAML file
        yaml_file: PathBuf,

        /// Path to output file. By default it prints to stdout
        out_path: Option<PathBuf>,
    },
    /// Generate Make dependency file listing dependencies for a YAML file.
    Makedeps {
        /// Input yaml file
        yaml_file: PathBuf,

        /// Dependencies output file
        deps_file: PathBuf,
    },
    /// Print list of all interrupts described by an SVD file
    Interrupts {
        /// Path to input SVD file
        svd_file: PathBuf,

        /// Whether to print gaps in interrupt number sequence
        #[clap(long)]
        no_gaps: bool,
    },
    /// Generate text-based memory map of an SVD file.
    Mmap {
        /// Path to input SVD file
        svd_file: PathBuf,
    },
    /// Convert SVD representation between file formats
    Convert {
        /// Path to input file
        in_path: PathBuf,

        /// Path to output file
        out_path: PathBuf,

        /// Format of input file (XML, JSON or YAML)
        #[clap(long = "input-format")]
        input_format: Option<convert_cli::InputFormat>,

        /// Format of output file (XML, JSON or YAML)
        #[clap(long = "output-format")]
        output_format: Option<convert_cli::OutputFormat>,

        /// Expand arrays, clusters and derived values
        #[clap(long)]
        expand: bool,

        /// Take size, access, reset_value, reset_mask from parents if absent in register
        #[clap(long)]
        expand_properties: bool,

        /// Skip enumeratedValues and writeConstraints during parsing (XML input only)
        #[clap(long)]
        ignore_enums: bool,

        /// Path to format config file
        ///
        /// If not specified, the default format config will be used.
        ///
        /// Only used for SVD output format.
        #[clap(long)]
        format_config: Option<PathBuf>,
    },
    /// Generates webpage with tables of existing peripherals
    Htmlcompare {
        /// Path to write HTML files to
        htmldir: PathBuf,

        /// Input SVD XML files
        svdfiles: Vec<PathBuf>,
    },
    /// Generates a webpage for a given SVD file containing details on every
    /// peripheral and register and their level of coverage.
    Html {
        /// Path to write HTML files to
        htmldir: PathBuf,

        /// Path to patched SVD files
        svdfiles: Vec<PathBuf>,
    },
    /// Prints informetion and statistics about SVD file
    Info {
        /// Path to input file
        in_path: PathBuf,
        /// Format of input file (XML, JSON or YAML)
        #[clap(long = "input-format")]
        input_format: Option<convert_cli::InputFormat>,
        /// Describe requested information
        request: String,
    },
}

impl Command {
    pub fn run(&self) -> Result<()> {
        match self {
            Self::Interrupts { svd_file, no_gaps } => {
                interrupts_cli::parse_device(svd_file, !no_gaps)?;
            }
            Self::Mmap { svd_file } => mmap_cli::parse_device(svd_file)?,
            Self::Patch {
                yaml_file,
                out_path,
                format_config,
                post_validate,
                show_patch_on_error,
                enum_derive,
            } => {
                let mut config = svdtools::patch::Config::default();
                if *post_validate {
                    config.post_validate = svd_rs::ValidateLevel::Strict;
                }
                config.show_patch_on_error = *show_patch_on_error;
                if let Some(enum_derive) = enum_derive.as_ref() {
                    config.enum_derive = *enum_derive;
                }
                patch_cli::patch(
                    yaml_file,
                    out_path.as_deref(),
                    format_config.as_deref(),
                    &config,
                )?
            }
            Self::ExpandPatch {
                yaml_file,
                out_path,
            } => {
                let yml = patch_cli::expand_patch(yaml_file)?;
                if let Some(out_path) = out_path.as_ref() {
                    let mut f = File::create(out_path)?;
                    f.write_all(yml.as_bytes())?;
                } else {
                    println!("{yml}");
                }
            }
            Self::Makedeps {
                yaml_file,
                deps_file,
            } => makedeps_cli::makedeps(yaml_file, deps_file)?,
            Self::Convert {
                in_path,
                out_path,
                input_format,
                output_format,
                expand,
                expand_properties,
                ignore_enums,
                format_config,
            } => convert_cli::convert(
                in_path,
                out_path,
                *input_format,
                *output_format,
                convert_cli::ParserConfig {
                    expand: *expand,
                    expand_properties: *expand_properties,
                    ignore_enums: *ignore_enums,
                },
                format_config.as_ref().map(|p| p.as_path()),
            )?,
            Self::Htmlcompare { htmldir, svdfiles } => {
                htmlcompare_cli::htmlcompare(htmldir, svdfiles)?;
            }
            Self::Html { htmldir, svdfiles } => {
                html_cli::svd2html(htmldir, svdfiles)?;
            }
            Self::Info {
                in_path,
                input_format,
                request,
            } => {
                let request = info::Request::from_str(request)?;
                let device = convert_cli::open_svd(
                    in_path,
                    *input_format,
                    convert_cli::ParserConfig {
                        ignore_enums: true,
                        ..Default::default()
                    },
                )?;
                let response = request.process(&device)?;
                println!("{response}")
            }
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
    use anyhow::Context;

    env_logger::init();

    let args = CliArgs::parse();
    if let Err(e) = args
        .command
        .run()
        .with_context(|| format!("by svdtools ({})", clap::crate_version!()))
    {
        log::error!("{:?}", e);

        std::process::exit(1);
    }
}
