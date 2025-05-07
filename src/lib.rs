use anyhow::anyhow;
use std::path::Path;
#[cfg(any(feature = "json", feature = "yaml"))]
use std::{fs::File, io::Read, str::FromStr};

pub mod common;
pub mod convert;
pub mod html;
pub mod info;
pub mod interrupts;
pub mod makedeps;
pub mod mmap;
pub mod patch;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigFormat {
    #[cfg(feature = "yaml")]
    Yaml,
    #[cfg(feature = "json")]
    Json,
}

#[cfg(any(feature = "json", feature = "yaml"))]
impl FromStr for ConfigFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "yaml")]
            "yml" | "yaml" | "YAML" => Ok(Self::Yaml),
            #[cfg(feature = "json")]
            "json" | "JSON" => Ok(Self::Json),
            _ => Err(anyhow!("Unknown config file format")),
        }
    }
}

#[cfg(any(feature = "json", feature = "yaml"))]
pub(crate) fn get_encoder_config(
    format_config: Option<&Path>,
) -> anyhow::Result<svd_encoder::Config> {
    Ok(if let Some(format_config) = format_config {
        let config_format = match format_config.extension().and_then(|e| e.to_str()) {
            Some(s) => ConfigFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown output file format")),
        };
        let mut config = String::new();
        File::open(format_config)?.read_to_string(&mut config)?;

        let config_map: std::collections::HashMap<String, String> = match config_format {
            #[cfg(feature = "yaml")]
            ConfigFormat::Yaml => serde_yaml::from_str(&config)?,
            #[cfg(feature = "json")]
            ConfigFormat::Json => serde_json::from_str(&config)?,
        };

        let mut config = svd_encoder::Config::default();
        config_map
            .iter()
            .for_each(|(name, value)| config.update(name, value));

        config
    } else {
        svd_encoder::Config::default()
    })
}

#[cfg(not(any(feature = "json", feature = "yaml")))]

pub(crate) fn get_encoder_config(
    format_config: Option<&Path>,
) -> anyhow::Result<svd_encoder::Config> {
    if let Some(format_config) = format_config {
        Err(anyhow!("In path: {}", format_config.display())
            .context("Config file passed to a build of svdtools that does not support it"))
    } else {
        Ok(svd_encoder::Config::default())
    }
}

#[cfg(test)]
mod test_utils;
