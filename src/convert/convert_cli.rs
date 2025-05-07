use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::str::FromStr;
use std::{fs::File, path::Path};
use svd_rs::Device;

use crate::get_encoder_config;
pub use crate::ConfigFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputFormat {
    Xml,
    #[cfg(feature = "yaml")]
    Yaml,
    #[cfg(feature = "json")]
    Json,
}

impl FromStr for InputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "svd" | "SVD" | "xml" | "XML" => Ok(Self::Xml),
            #[cfg(feature = "yaml")]
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            #[cfg(feature = "json")]
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown input file format")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    Xml,
    #[cfg(feature = "yaml")]
    Yaml,
    #[cfg(feature = "json")]
    Json,
}

impl FromStr for OutputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "svd" | "SVD" | "xml" | "XML" => Ok(Self::Xml),
            #[cfg(feature = "yaml")]
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            #[cfg(feature = "json")]
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown output file format")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ParserConfig {
    pub expand: bool,
    pub expand_properties: bool,
    pub ignore_enums: bool,
}

pub fn open_svd(
    in_path: &Path,
    input_format: Option<InputFormat>,
    parser_config: ParserConfig,
) -> Result<Device> {
    let input_format = match input_format {
        None => match in_path.extension().and_then(|e| e.to_str()) {
            Some(s) => InputFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown input file format")),
        },
        Some(t) => t,
    };

    let mut input = String::new();
    File::open(in_path)?.read_to_string(&mut input)?;

    let mut device = match input_format {
        InputFormat::Xml => svd_parser::parse_with_config(
            &input,
            &svd_parser::Config::default().ignore_enums(parser_config.ignore_enums),
        )?,
        #[cfg(feature = "yaml")]
        InputFormat::Yaml => serde_yaml::from_str(&input)?,
        #[cfg(feature = "json")]
        InputFormat::Json => serde_json::from_str(&input)?,
    };
    if parser_config.expand_properties {
        svd_parser::expand_properties(&mut device);
    }
    let device = if parser_config.expand {
        svd_parser::expand(&device)?
    } else {
        device
    };
    Ok(device)
}

pub fn convert(
    in_path: &Path,
    out_path: &Path,
    input_format: Option<InputFormat>,
    output_format: Option<OutputFormat>,
    parser_config: ParserConfig,
    format_config: Option<&Path>,
) -> Result<()> {
    let device = open_svd(in_path, input_format, parser_config)?;

    let output_format = match output_format {
        None => match out_path.extension().and_then(|e| e.to_str()) {
            Some(s) => OutputFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown output file format")),
        },
        Some(t) => t,
    };

    let config = get_encoder_config(format_config)?;

    let output = match output_format {
        OutputFormat::Xml => svd_encoder::encode_with_config(&device, &config)?,
        #[cfg(feature = "yaml")]
        OutputFormat::Yaml => serde_yaml::to_string(&device)?,
        #[cfg(feature = "json")]
        OutputFormat::Json => serde_json::to_string_pretty(&device)?,
    };

    File::create(out_path)?.write_all(output.as_bytes())?;

    Ok(())
}
