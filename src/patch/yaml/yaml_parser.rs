use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize};
use serde_yaml::Mapping;
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use svd_parser as svd;

#[derive(Debug, Deserialize)]
pub struct YamlRoot {
    #[serde(rename = "_svd")]
    pub svd: PathBuf,

    #[serde(flatten)]
    pub body: YamlBody,
}

#[derive(Debug, Deserialize)]
pub struct YamlBody {
    #[serde(flatten)]
    pub commands: PeripheralCommand,

    #[serde(flatten)]
    pub peripherals: HashMap<String, PeripheralNode>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Peripheral {
    pub name: Option<String>,

    #[serde(flatten)]
    pub body: PeripheralBody,

    pub address_block: Option<OptAddressBlock>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AddedPeripheral {
    #[serde(flatten)]
    pub body: PeripheralBody,

    pub address_block: Option<svd::AddressBlock>,

    #[serde(default)]
    interrupts: HashMap<String, InterruptBody>,

    // TODO handle addressBlocks? they are not used in stm32-rs
    derived_from: Option<String>,
}

impl AddedPeripheral {
    fn interrupts(&self) -> Vec<svd::Interrupt> {
        self.interrupts
            .iter()
            .map(|i| {
                let name = i.0;
                let body = i.1;
                svd::Interrupt {
                    name: name.clone(),
                    description: body.description.clone(),
                    value: body.value,
                }
            })
            .collect()
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct InterruptBody {
    description: Option<String>,
    value: u32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PeripheralBody {
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub group_name: Option<String>,
    pub base_address: Option<u32>,
    pub registers: Option<Vec<Register>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OptAddressBlock {
    pub offset: Option<u32>,
    pub size: Option<u32>,
    pub usage: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Register {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub address_offset: Option<String>,
    pub size: Option<String>,
    pub access: Option<String>,
    pub reset_value: Option<String>,
    pub fields: Option<Vec<Field>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Field {
    pub name: Option<String>,
    pub description: Option<String>,
    pub bit_offset: Option<u32>,
    pub bit_width: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PeripheralNode {
    #[serde(flatten)]
    pub commands: RegisterCommand,

    #[serde(flatten)]
    pub registers: HashMap<String, RegisterNode>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegisterNode {
    #[serde(flatten)]
    pub commands: FieldCommand,
}

// TODO after that riir is complete, this should be rewritten by remembering
//      the ordering of the commands.
//      See https://github.com/stm32-rs/svdtools/issues/9#issuecomment-605467243
#[derive(Debug, Deserialize)]
pub struct PeripheralCommand {
    #[serde(default, rename = "_include")]
    pub include: Vec<PathBuf>,

    #[serde(default, rename = "_delete")]
    pub delete: Vec<String>,

    #[serde(rename = "_modify")]
    pub modify: Option<Device>,

    #[serde(default, rename = "_add")]
    pub add: HashMap<String, AddedPeripheral>,

    /// Copy everything except `baseAddress` and `name` from another peripheral
    #[serde(default, rename = "_copy")]
    pub copy: HashMap<String, CopySource>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub address_unit_bits: Option<u32>,
    pub width: Option<u32>,
    pub cpu: Option<Cpu>,

    #[serde(flatten)]
    pub default_register_properties: RegisterProperties,

    #[serde(flatten)]
    pub peripherals: HashMap<String, Peripheral>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegisterProperties {
    pub size: Option<u32>,
    pub reset_value: Option<u32>,
    pub reset_mask: Option<u32>,
    pub access: Option<Access>,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Access {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "read-write")]
    ReadWrite,
    #[serde(rename = "read-writeOnce")]
    ReadWriteOnce,
    #[serde(rename = "writeOnce")]
    WriteOnce,
    #[serde(rename = "write-only")]
    WriteOnly,
}

impl Access {
    pub fn to_svd(&self) -> svd::Access {
        match self {
            Self::ReadOnly => svd::Access::ReadOnly,
            Self::ReadWrite => svd::Access::ReadWrite,
            Self::ReadWriteOnce => svd::Access::ReadWriteOnce,
            Self::WriteOnce => svd::Access::WriteOnce,
            Self::WriteOnly => svd::Access::WriteOnly,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Cpu {
    pub name: Option<String>,

    /// HW revision
    pub revision: Option<String>,

    /// Endianness
    pub endian: Option<svd::Endian>,

    /// Indicate whether the processor is equipped with a memory protection unit (MPU)
    pub mpu_present: Option<bool>,

    /// Indicate whether the processor is equipped with a hardware floating point unit (FPU)
    pub fpu_present: Option<bool>,

    /// Number of bits available in the Nested Vectored Interrupt Controller (NVIC) for configuring priority
    pub nvic_prio_bits: Option<u32>,

    /// Indicate whether the processor implements a vendor-specific System Tick Timer
    pub vendor_systick_config: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CopySource {
    pub from: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FieldCommand {
    #[serde(default, rename = "_delete")]
    pub delete: Vec<String>,

    #[serde(default, rename = "_merge")]
    pub merge: Vec<String>,

    #[serde(default, rename = "_modify")]
    pub modify: HashMap<String, Field>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegisterCommand {
    #[serde(default, rename = "_include")]
    pub include: Vec<PathBuf>,

    #[serde(default, rename = "_delete")]
    pub delete: Vec<String>,

    #[serde(rename = "_modify")]
    pub modify: Option<Register>,

    #[serde(default, rename = "_add")]
    pub add: Mapping,
}

pub fn from_path<T>(yaml_file: &Path) -> T
where
    T: DeserializeOwned,
{
    let file = File::open(yaml_file).expect("yaml file doesn't exist");
    let reader = BufReader::new(file);
    serde_yaml::from_reader(reader).expect("yaml not formatted correctly")
}

impl AddedPeripheral {
    pub fn to_svd(&self, peripheral_name: &str) -> Result<svd::Peripheral> {
        let mut pb: svd::peripheral::PeripheralBuilder = Default::default();
        pb = pb.name(peripheral_name.to_string());
        if let Some(base_address) = &self.body.base_address {
            pb = pb.base_address(*base_address);
        }
        pb = pb.version(self.body.version.clone());
        pb = pb.display_name(self.body.display_name.clone());
        pb = pb.group_name(self.body.group_name.clone());
        pb = pb.description(self.body.description.clone());
        pb = pb.address_block(self.address_block.clone());

        pb = pb.interrupt(self.interrupts());

        // TODO pb = pb.registers(self.body.registers);
        pb = pb.derived_from(self.derived_from.clone());

        pb.build()
    }
}
