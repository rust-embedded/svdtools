use quick_xml::de;
use serde::Deserialize;
use std::io::{BufReader, Read};

#[derive(Deserialize, Debug)]
struct Svd {
    peripherals: PeripheralList,
}

#[derive(Deserialize, Debug)]
struct PeripheralList {
    peripheral: Vec<Peripheral>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Peripheral {
    /// attribute
    pub derived_from: Option<String>,
    pub name: String,
    pub base_address: String,
    // some peripherals have no interrupts
    pub interrupt: Option<Vec<Interrupt>>,
    pub registers: Option<RegisterList>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Interrupt {
    pub name: String,
    pub description: Option<String>,
    pub value: u32,
}

#[derive(Deserialize, Debug)]
pub struct RegisterList {
    pub register: Vec<Register>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Register {
    pub name: String,
    pub description: Option<String>,
    pub address_offset: String,
    pub access: Option<String>,
    pub fields: FieldList,
}

#[derive(Deserialize, Debug)]
pub struct FieldList {
    pub field: Option<Vec<Field>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub description: Option<String>,
    pub bit_offset: u32,
    pub bit_width: u32,
    pub access: Option<String>,
}

pub fn peripherals_with_interrupts<R: Read>(svd: R) -> Vec<Peripheral> {
    let reader = BufReader::new(svd);
    let svd: Svd = de::from_reader(reader).expect("svd not formatted correctly");

    svd.peripherals.peripheral
}
