use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::str::FromStr;
use std::{fs::File, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputFormat {
    Xml,
    Yaml,
    Json,
}

impl FromStr for InputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "svd" | "SVD" | "xml" | "XML" => Ok(Self::Xml),
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown input file format")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    Xml,
    Yaml,
    Json,
}

impl FromStr for OutputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "svd" | "SVD" | "xml" | "XML" => Ok(Self::Xml),
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown output file format")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigFormat {
    Yaml,
    Json,
}

impl FromStr for ConfigFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown config file format")),
        }
    }
}

pub struct ParserConfig {
    pub expand: bool,
    pub expand_properties: bool,
    pub ignore_enums: bool,
}

pub fn convert(
    in_path: &Path,
    out_path: &Path,
    input_format: Option<InputFormat>,
    output_format: Option<OutputFormat>,
    parser_config: ParserConfig,
    format_config: Option<&Path>,
) -> Result<()> {
    let input_format = match input_format {
        None => match in_path.extension().and_then(|e| e.to_str()) {
            Some(s) => InputFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown input file format")),
        },
        Some(t) => t,
    };
    let output_format = match output_format {
        None => match out_path.extension().and_then(|e| e.to_str()) {
            Some(s) => OutputFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown output file format")),
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
        InputFormat::Yaml => serde_yaml::from_str(&input)?,
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

    let config = if let Some(format_config) = format_config {
        let config_format = match format_config.extension().and_then(|e| e.to_str()) {
            Some(s) => ConfigFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown output file format")),
        };
        let mut config = String::new();
        File::open(format_config)?.read_to_string(&mut config)?;

        let config_map: std::collections::HashMap<String, String> = match config_format {
            ConfigFormat::Yaml => serde_yaml::from_str(&config)?,
            ConfigFormat::Json => serde_json::from_str(&config)?,
        };

        let mut config = svd_encoder::Config::default();
        config_map
            .iter()
            .for_each(|(name, value)| config.update(name, value));

        config
    } else {
        svd_encoder::Config::default()
    };

    let output = match output_format {
        OutputFormat::Xml => svd_encoder::encode_with_config(&device, &config)?,
        OutputFormat::Yaml => serde_yaml::to_string(&device)?,
        OutputFormat::Json => serde_json::to_string_pretty(&device)?,
    };

    File::create(out_path)?.write_all(output.as_bytes())?;

    Ok(())
}
