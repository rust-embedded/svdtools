use serde::{de::DeserializeOwned, Deserialize};
use serde_yaml::Mapping;
use std::{
    collections::HashMap,
    fs::File,
    hash::Hash,
    io::BufReader,
    path::{Path, PathBuf},
};

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
    name: Option<String>,
    description: Option<String>,
    group_name: Option<String>,
    base_address: Option<String>,
    address_block: Option<Mapping>,
    registers: Option<Vec<Register>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Register {
    name: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    address_offset: Option<String>,
    size: Option<String>,
    access: Option<String>,
    reset_value: Option<String>,
    fields: Option<Vec<Field>>,
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
    pub modify: Option<ModifyPeripheral>,

    #[serde(default, rename = "_add")]
    pub add: Mapping,

    /// Copy everything except `baseAddress` and `name` from another peripheral
    #[serde(default, rename = "_copy")]
    pub copy: HashMap<String, CopySource>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModifyPeripheral {
    cpu: Option<Cpu>,

    #[serde(flatten)]
    pub peripherals: HashMap<String, Peripheral>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Cpu {
    pub name: Option<String>,

    /// HW revision
    pub revision: Option<String>,

    /// Endianness
    // TODO enum
    pub endian: Option<String>,

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

impl Merge for YamlBody {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
        merge_hashmap(&mut self.peripherals, &other.peripherals)
    }
}

impl Merge for PeripheralCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        merge_option(&mut self.modify, &other.modify);
        // TODO merge add
        // TODO merge copy
    }
}

impl Merge for ModifyPeripheral {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.cpu, &other.cpu);
        merge_hashmap(&mut self.peripherals, &other.peripherals);
    }
}

impl Merge for PeripheralNode {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
        merge_hashmap(&mut self.registers, &other.registers);
    }
}

impl Merge for RegisterNode {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
    }
}

impl Merge for RegisterCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        merge_opt_struct(&mut self.modify, &other.modify);
        // TODO merge add
    }
}

impl Merge for FieldCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        self.merge.extend(other.merge.clone());

        merge_hashmap(&mut self.modify, &other.modify);
    }
}

impl Merge for Peripheral {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.group_name, &other.group_name);
        merge_option(&mut self.base_address, &other.base_address);
        merge_option(&mut self.address_block, &other.address_block);
        merge_opt_vec(&mut self.registers, &other.registers)
    }
}

impl Merge for Register {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.display_name, &other.display_name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.address_offset, &other.address_offset);
        merge_option(&mut self.size, &other.size);
        merge_option(&mut self.access, &other.access);
        merge_option(&mut self.reset_value, &other.reset_value);
        merge_opt_vec(&mut self.fields, &other.fields)
    }
}

impl Merge for Field {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.bit_offset, &other.bit_offset);
        merge_option(&mut self.bit_width, &other.bit_width);
    }
}

impl Merge for Cpu {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.revision, &other.revision);
        merge_option(&mut self.endian, &other.endian);
        merge_option(&mut self.mpu_present, &other.mpu_present);
        merge_option(&mut self.fpu_present, &other.fpu_present);
        merge_option(&mut self.nvic_prio_bits, &other.nvic_prio_bits);
        merge_option(
            &mut self.vendor_systick_config,
            &other.vendor_systick_config,
        );
    }
}

fn merge_hashmap<K, V>(dest: &mut HashMap<K, V>, src: &HashMap<K, V>)
where
    K: Eq + Hash + Clone,
    V: Clone + Merge,
{
    for (key, value) in src {
        let corresponding = dest.get_mut(key);
        if let Some(entry) = corresponding {
            entry.merge(value);
        } else {
            dest.insert(key.clone(), value.clone());
        }
    }
}

fn merge_opt_vec<T: Clone + Merge>(dest: &mut Option<Vec<T>>, src: &Option<Vec<T>>) {
    if let Some(src) = src {
        let mut src = src.clone();
        match dest {
            Some(dest) => dest.append(&mut src),
            None => *dest = Some(src),
        }
    }
}

fn merge_opt_struct<T: Clone + Merge>(dest: &mut Option<T>, src: &Option<T>) {
    if let Some(src) = src {
        match dest {
            Some(dest) => dest.merge(src),
            None => *dest = Some(src.clone()),
        }
    }
}

fn merge_option<T: Clone>(dest: &mut Option<T>, src: &Option<T>) {
    if dest.is_none() && src.is_some() {
        *dest = src.clone();
    }
}

pub trait Merge {
    fn merge(&mut self, other: &Self);
}
