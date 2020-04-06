use serde::{de::DeserializeOwned, Deserialize};
use serde_yaml::{Mapping, Value};
use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Deserialize)]
pub struct Root {
    #[serde(rename = "_svd")]
    pub svd: String,

    #[serde(flatten)]
    pub body: Node,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    #[serde(rename = "_include")]
    pub include: Option<Vec<String>>,

    #[serde(rename = "_delete")]
    pub delete: Option<Vec<String>>,

    #[serde(rename = "_modify")]
    pub modify: Option<Mapping>,

    #[serde(flatten)]
    pub data: Option<Mapping>,
}

pub fn from_path<T>(yaml_file: &Path) -> T
where
    T: DeserializeOwned,
{
    let file = File::open(yaml_file).expect("yaml file doesn't exist");
    let reader = BufReader::new(file);
    serde_yaml::from_reader(reader).expect("yaml not formatted correctly")
}
