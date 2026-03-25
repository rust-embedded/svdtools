use anyhow::{anyhow, Result};
use std::str::FromStr;

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
