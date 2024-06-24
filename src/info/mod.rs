use std::str::FromStr;

use anyhow::Ok;
use svd_rs::Device;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Request {
    DeviceName,
}

impl FromStr for Request {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "device-name" => Ok(Self::DeviceName),
            _ => Err(anyhow::anyhow!("Unknown info request: {s}")),
        }
    }
}

impl Request {
    pub fn process(&self, device: &Device) -> anyhow::Result<String> {
        match self {
            Self::DeviceName => Ok(device.name.to_string()),
        }
    }
}
