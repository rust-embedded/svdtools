use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;
use serde::Deserialize;
use quick_xml::de;

#[derive(Deserialize, Debug)]
struct Svd {
    peripherals: PeripheralList,
}

#[derive(Deserialize, Debug)]
struct PeripheralList {
    peripheral: Vec<Peripheral>,
}

#[derive(Deserialize, Debug)]
struct Peripheral {
    name: String,
    interrupt: Option<Vec<Interrupt>>,
}

#[derive(Deserialize, Debug)]
struct Interrupt {
    name: String,
    description: String,
    value: u32,
}

pub fn parse_device(svd_file: PathBuf, gaps: bool) {
    println!("{:?}, {}", svd_file, gaps);
    let file = File::open(svd_file).expect("svd file doesn't exist");
    let reader = BufReader::new(file);

    let peripherals: Svd = de::from_reader(reader).expect("svd not formatted correctly");

    println!("{:#?}", peripherals);
}
