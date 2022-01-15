use anyhow::Result;
use std::{fs::File, io::Read, path::Path};
use svd_parser::svd::{Device, Peripheral};

pub fn peripherals<R: Read>(svd: &mut R) -> Result<Vec<Peripheral>> {
    let xml = &mut String::new();
    svd.read_to_string(xml).unwrap();
    let device = parse_device(xml)?;
    Ok(device.peripherals)
}

fn parse_device(xml: &str) -> Result<Device> {
    svd_parser::parse(xml)
}

pub fn device(path: &Path) -> Result<Device> {
    let xml = &mut String::new();
    let mut svd_file = File::open(path).expect("svd path is not correct");
    svd_file.read_to_string(xml).unwrap();
    parse_device(xml)
}
