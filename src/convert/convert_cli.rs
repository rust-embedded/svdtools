use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::str::FromStr;
use std::{fs::File, path::Path};

#[derive(Clone, Copy, Debug, PartialEq)]
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
            _ => return Err(anyhow!("Unknown input file format")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
            _ => return Err(anyhow!("Unknown output file format")),
        }
    }
}

pub fn convert(
    in_path: &Path,
    out_path: &Path,
    input_format: Option<InputFormat>,
    output_format: Option<OutputFormat>,
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
        InputFormat::Xml => svd_parser::parse(&input)?,
        InputFormat::Yaml => serde_yaml::from_str(&input)?,
        InputFormat::Json => serde_json::from_str(&input)?,
    };

    if device.schema_version == "" {
        device.schema_version = "1.1".into();
    }
    if device.xmlns_xs == "" {
        device.xmlns_xs = "http://www.w3.org/2001/XMLSchema-instance".into();
    }
    if device.no_namespace_schema_location == "" {
        device.no_namespace_schema_location = format!(
            "CMSIS-SVD_Schema_{}.xsd",
            device.schema_version.replace(".", "_")
        );
    }

    let output = match output_format {
        OutputFormat::Xml => svd_encoder::encode(&device)?,
        OutputFormat::Yaml => serde_yaml::to_string(&device)?,
        OutputFormat::Json => serde_json::to_string_pretty(&device)?,
    };

    File::create(out_path)?.write_all(output.as_bytes())?;

    Ok(())
}
