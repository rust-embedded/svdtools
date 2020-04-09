use serde::{de::DeserializeOwned, Deserialize};
use serde_yaml::Mapping;
use std::{
    collections::HashMap,
    fs::File,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peripheral {
    name: Option<String>,
    description: Option<String>,
    group_name: Option<String>,
    base_address: Option<String>,
    address_block: Option<Mapping>,
    registers: Option<Vec<Register>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Register {
    name: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    address_offset: Option<String>,
    size: Option<String>,
    access: Option<String>,
    reset_value: Option<String>,
    fiels: Option<Vec<Field>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    name: Option<String>,
    description: Option<String>,
    bit_offset: Option<String>,
    bit_width: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PeripheralData {
    #[serde(flatten)]
    pub peripherals: HashMap<String, RegisterData>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterData {
    #[serde(flatten)]
    pub registers: Mapping,
}

#[derive(Debug, Deserialize)]
pub struct PeripheralNode {
    #[serde(flatten)]
    pub commands: RegisterCommand,

    #[serde(flatten)]
    pub registers: HashMap<String, RegisterNode>,
}

#[derive(Debug, Deserialize)]
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

    #[serde(default, rename = "_modify")]
    pub modify: HashMap<String, Peripheral>,

    #[serde(default, rename = "_add")]
    pub add: Mapping,
}

#[derive(Debug, Deserialize)]
pub struct FieldCommand {
    #[serde(default, rename = "_delete")]
    pub delete: Vec<String>,

    #[serde(default, rename = "_merge")]
    pub merge: Vec<String>,

    #[serde(default, rename = "_modify")]
    pub modify: HashMap<String, Field>,
}

#[derive(Debug, Deserialize)]
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

impl YamlBody {
    pub fn merge(&mut self, child: &YamlBody) {
        todo!()
    }
}

impl PeripheralNode {
    pub fn merge(&mut self, child: &PeripheralNode) {
        todo!()
    }
}
