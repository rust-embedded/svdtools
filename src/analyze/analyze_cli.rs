use crate::convert::convert_cli::InputFormat;
use anyhow::{anyhow, Result};
use log::info;
use std::io::Read;
use std::str::FromStr;
use std::{fs::File, path::Path};
use svd_rs::{
    ClusterInfo, Device, FieldInfo, MaybeArray, PeripheralInfo, RegisterCluster, RegisterInfo,
};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct CompareConfig {
    pub compare_description: bool,
    pub with_fields: bool,
}

pub trait Same {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool;
}

impl Same for PeripheralInfo {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        if self.derived_from.is_some() || other.derived_from.is_some() {
            return false;
        }
        (!config.compare_description || self.description == other.description)
            && self
                .registers
                .as_deref()
                .is_copy(&other.registers.as_deref(), config)
    }
}

impl Same for ClusterInfo {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        if self.derived_from.is_some() || other.derived_from.is_some() {
            return false;
        }
        (!config.compare_description || self.description == other.description)
            && self.children.is_copy(&other.children, config)
    }
}

impl Same for RegisterInfo {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        if self.derived_from.is_some() || other.derived_from.is_some() {
            return false;
        }
        (!config.compare_description || self.description == other.description)
            && self.modified_write_values == other.modified_write_values
            && self.properties == other.properties
            && self.write_constraint == other.write_constraint
            && self.read_action == other.read_action
            && self
                .fields
                .as_deref()
                .is_copy(&other.fields.as_deref(), config)
    }
}

impl Same for RegisterCluster {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        match (self, other) {
            (Self::Register(s), Self::Register(o)) if s.is_copy(o, config) => true,
            (Self::Cluster(s), Self::Cluster(o)) if s.is_copy(o, config) => true,
            _ => false,
        }
    }
}

impl Same for FieldInfo {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        if self.derived_from.is_some() || other.derived_from.is_some() {
            return false;
        }
        (!config.compare_description || self.description == other.description)
            && self.bit_width() == other.bit_width()
            && self.modified_write_values == other.modified_write_values
            && self.access == other.access
            && self.write_constraint == other.write_constraint
            && self.read_action == other.read_action
            && self.enumerated_values == other.enumerated_values
    }
}

impl<T: Same> Same for MaybeArray<T> {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        match (self, other) {
            (Self::Array(sinfo, sdim), Self::Array(oinfo, odim))
                if sdim == odim && sinfo.is_copy(oinfo, config) =>
            {
                true
            }
            (Self::Single(sinfo), Self::Single(oinfo)) if sinfo.is_copy(oinfo, config) => true,
            _ => false,
        }
    }
}

impl<T: Same + ?Sized> Same for Option<&T> {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        match (self, other) {
            (Some(s), Some(o)) if s.is_copy(o, config) => true,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Same> Same for [T] {
    fn is_copy(&self, other: &Self, config: &CompareConfig) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (s, o) in self.iter().zip(other.iter()) {
            if !s.is_copy(o, config) {
                return false;
            }
        }
        true
    }
}

pub fn analyze(device: &Device, config: &CompareConfig) {
    let mut pcopies = Vec::new();
    for (i, p1) in device.peripherals.iter().enumerate() {
        for j in (i + 1)..device.peripherals.len() {
            let p2 = &device.peripherals[j];
            if p2.is_copy(p1, &config) {
                info!("Peripheral {} == {}", &p2.name, &p1.name);
                pcopies.push(p2);
                break;
            }
        }
    }
    for p in &device.peripherals {
        if pcopies.contains(&p) {
            continue;
        }
        let mut rcopies = Vec::new();
        let all_registers = p.all_registers().collect::<Vec<_>>();
        for (i, &r1) in all_registers.iter().enumerate() {
            for j in (i + 1)..all_registers.len() {
                let r2 = all_registers[j];
                if r2.is_copy(r1, &config) {
                    info!(
                        "In peripheral {}: register {} == {}",
                        &p.name, &r1.name, &r2.name
                    );
                    rcopies.push(r2);
                    break;
                }
            }
        }
        if config.with_fields {
            for r in all_registers {
                if rcopies.contains(&r) {
                    continue;
                }
                if let Some(fields) = r.fields.as_ref() {
                    for (i, f1) in fields.iter().enumerate() {
                        for j in (i + 1)..fields.len() {
                            let f2 = &fields[j];
                            if f2.is_copy(f1, &config) {
                                info!(
                                    "In register {}.{}: field {} == {}",
                                    &p.name, &r.name, &f1.name, &f2.name
                                );
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn analyze_file(
    in_path: &Path,
    input_format: Option<InputFormat>,
    config: &CompareConfig,
) -> Result<()> {
    let input_format = match input_format {
        None => match in_path.extension().and_then(|e| e.to_str()) {
            Some(s) => InputFormat::from_str(s)?,
            _ => return Err(anyhow!("Unknown input file format")),
        },
        Some(t) => t,
    };

    let mut input = String::new();
    File::open(in_path)?.read_to_string(&mut input)?;

    let device = match input_format {
        InputFormat::Xml => svd_parser::parse(&input)?,
        InputFormat::Yaml => serde_yaml::from_str(&input)?,
        InputFormat::Json => serde_json::from_str(&input)?,
    };

    analyze(&device, config);

    Ok(())
}
