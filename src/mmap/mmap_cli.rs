use super::svd_reader;
use crate::common::str_utils;
use std::{fs::File, path::Path};
use svd_reader::{Peripheral, Register};

pub fn parse_device(svd_file: &Path) {
    let file = File::open(svd_file).expect("svd file doesn't exist");
    let peripherals = svd_reader::peripherals_with_interrupts(file);

    let text = to_text(&peripherals);
    println!("{}", text);
}

fn to_text(peripherals: &[Peripheral]) -> String {
    let mut mmap: Vec<String> = vec![];

    for p in peripherals {
        get_peripheral(&p, &mut mmap);
        get_interrupts(&p, &mut mmap);
        get_registers(&p, &mut mmap);
    }

    mmap.sort();
    mmap.join("\n")
}

fn get_peripheral(peripheral: &Peripheral, mmap: &mut Vec<String>) {
    let text = format!(
        "{} A PERIPHERAL {}",
        str_utils::format_address(&peripheral.base_address),
        peripheral.name
    );
    mmap.push(text);
}

fn get_interrupts(peripheral: &Peripheral, mmap: &mut Vec<String>) {
    if let Some(interrupts) = &peripheral.interrupt {
        for i in interrupts {
            let description = str_utils::get_description(&i.description);
            let text = format!(
                "INTERRUPT {:03}: {} ({}): {}",
                i.value, i.name, peripheral.name, description
            );
            mmap.push(text);
        }
    }
}

fn get_registers(peripheral: &Peripheral, mmap: &mut Vec<String>) {
    if let Some(registers) = &peripheral.registers {
        for r in &registers.register {
            let description = str_utils::get_description(&r.description);
            let access = str_utils::access_with_brace(&r.access);
            let addr = sum_address(&peripheral.base_address, &r.address_offset);
            let text = format!("{} B  REGISTER {}{}: {}", addr, r.name, access, description);
            mmap.push(text);
            get_fields(r, &addr, mmap)
        }
    }
}

fn get_fields(register: &Register, addr: &str, mmap: &mut Vec<String>) {
    if let Some(fields) = &register.fields.field {
        for f in fields {
            let description = str_utils::get_description(&f.description);
            let access = str_utils::access_with_brace(&f.access);
            let text = format!(
                "{} C   FIELD {:02}w{:02} {}{}: {}",
                addr, f.bit_offset, f.bit_width, f.name, access, description
            );
            mmap.push(text);
        }
    }
}

fn sum_address(addr1: &str, addr2: &str) -> String {
    let addr1 = to_unsigned(addr1);
    let addr2 = to_unsigned(addr2);
    let out = addr1 + addr2;
    let out = format! {"0x{:x}", out};
    str_utils::format_address(&out)
}

fn to_unsigned(hex_addr: &str) -> u64 {
    let addr = hex_addr.trim_start_matches("0x");
    u64::from_str_radix(addr, 16).expect("bad address")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correctly_sum_address() {
        let addr1 = "0x4000";
        let addr2 = "0x8";
        let expected_addr = "0x4008";
        let actual_addr = sum_address(addr1, addr2);
        assert_eq!(expected_addr, &actual_addr);
    }
}
