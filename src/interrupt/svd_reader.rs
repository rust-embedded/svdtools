use quick_xml::de;
use serde::Deserialize;
use std::io::{BufReader, Read};

#[derive(Deserialize, Debug)]
struct Svd {
    peripherals: PeripheralList,
}

#[derive(Deserialize, Debug)]
struct PeripheralList {
    peripheral: Vec<PeripheralXml>,
}

#[derive(Deserialize, Debug)]
struct PeripheralXml {
    name: String,
    // some peripherals have no interrupts
    interrupt: Option<Vec<Interrupt>>,
}

#[derive(Deserialize, Debug)]
pub struct Interrupt {
    pub name: String,
    pub description: Option<String>,
    pub value: u32,
}

pub struct Peripheral {
    pub name: String,
    pub interrupt: Vec<Interrupt>,
}

/// get all peripherals that contain at least one interrupt
pub fn peripherals_with_interrupts<R: Read>(svd: R) -> impl Iterator<Item = Peripheral> {
    let reader = BufReader::new(svd);
    let svd: Svd = de::from_reader(reader).expect("svd not formatted correctly");

    let peripheral_list = svd.peripherals.peripheral;

    peripheral_list.into_iter().filter_map(|p| {
        if let Some(interrupt) = p.interrupt {
            Some(Peripheral {
                name: p.name,
                interrupt,
            })
        } else {
            None
        }
    })
}
