use std::io::Read;
use svd_parser::Peripheral;

pub fn peripherals<R: Read>(svd: &mut R) -> Vec<Peripheral> {
    let xml = &mut String::new();
    svd.read_to_string(xml).unwrap();
    let device = svd_parser::parse(xml).expect("svd not formatted correctly");
    device.peripherals
}
