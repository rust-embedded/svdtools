use serde::{de::DeserializeOwned, Deserialize};
use serde_yaml::Mapping;
use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Deserialize)]
pub struct Root {
    #[serde(rename = "_svd")]
    pub svd: String,

    #[serde(flatten)]
    pub body: RootNode,
}

#[derive(Debug, Deserialize)]
pub struct PeripheralNode {
    #[serde(flatten)]
    pub data: Mapping,
}

// TODO after that riir is complete, this should be rewritten by remembering
//      the ordering of the commands.
//      See https://github.com/stm32-rs/svdtools/issues/9#issuecomment-605467243
#[derive(Debug, Deserialize)]
pub struct Command {
    #[serde(rename = "_include")]
    pub include: Option<Vec<String>>,

    #[serde(rename = "_delete")]
    pub delete: Option<Vec<String>>,

    #[serde(rename = "_modify")]
    pub modify: Option<Mapping>,
}

#[derive(Debug, Deserialize)]
pub struct RootNode {
    #[serde(flatten)]
    pub commands: Command,

    #[serde(flatten)]
    pub peripherals: PeripheralNode,
}

pub fn from_path<T>(yaml_file: &Path) -> T
where
    T: DeserializeOwned,
{
    let file = File::open(yaml_file).expect("yaml file doesn't exist");
    let reader = BufReader::new(file);
    serde_yaml::from_reader(reader).expect("yaml not formatted correctly")
}
