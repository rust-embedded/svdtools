use crate::common::{str_utils, svd_utils};
use crate::mmap::svd_reader;
use std::{fs::File, path::Path};
use svd_parser::{Peripheral, Register, RegisterCluster};

pub fn parse_device(svd_file: &Path) {
    let mut file = File::open(svd_file).expect("svd file doesn't exist");
    let peripherals = svd_reader::peripherals(&mut file);

    let text = to_text(&peripherals);
    println!("{}", text);
}

fn to_text(peripherals: &[Peripheral]) -> String {
    let mut mmap: Vec<String> = vec![];

    for p in peripherals {
        get_peripheral(&p, &mut mmap);
        get_interrupts(&p, &mut mmap);
        let registers = get_periph_registers(p, peripherals);
        get_registers(p.base_address, registers, &mut mmap);
    }

    mmap.sort();
    mmap.join("\n")
}

fn get_periph_registers<'a>(
    peripheral: &'a Peripheral,
    peripheral_list: &'a [Peripheral],
) -> &'a Option<Vec<RegisterCluster>> {
    match &peripheral.derived_from {
        None => &peripheral.registers,
        Some(father) => {
            let mut registers = &None;
            for p in peripheral_list {
                if &p.name == father {
                    registers = &p.registers;
                }
            }
            registers
        }
    }
}

fn get_peripheral(peripheral: &Peripheral, mmap: &mut Vec<String>) {
    let text = format!(
        "{} A PERIPHERAL {}",
        str_utils::format_address(peripheral.base_address),
        peripheral.name
    );
    mmap.push(text);
}

fn get_interrupts(peripheral: &Peripheral, mmap: &mut Vec<String>) {
    for i in &peripheral.interrupt {
        let description = str_utils::get_description(&i.description);
        let text = format!(
            "INTERRUPT {:03}: {} ({}): {}",
            i.value, i.name, peripheral.name, description
        );
        mmap.push(text);
    }
}

fn get_registers(
    base_address: u32,
    registers: &Option<Vec<RegisterCluster>>,
    mmap: &mut Vec<String>,
) {
    if let Some(registers) = registers {
        for r in registers {
            match &r {
                RegisterCluster::Register(r) => {
                    let description = str_utils::get_description(&r.description);
                    let access = svd_utils::access_with_brace(&r.access);
                    let addr = base_address + &r.address_offset;
                    let addr = str_utils::format_address(addr);
                    let text =
                        format!("{} B  REGISTER {}{}: {}", addr, r.name, access, description);
                    mmap.push(text);
                    get_fields(r, &addr, mmap)
                }
                RegisterCluster::Cluster(c) => {
                    let description = str_utils::get_description(&c.description);
                    let addr = base_address + &c.address_offset;
                    format!("{} B  CLUSTER {}: {}", addr, c.name, description);
                }
            }
        }
    }
}

fn get_fields(register: &Register, addr: &str, mmap: &mut Vec<String>) {
    if let Some(fields) = &register.fields {
        for f in fields {
            let description = str_utils::get_description(&f.description);
            let access = svd_utils::access_with_brace(&f.access);
            let text = format!(
                "{} C   FIELD {:02}w{:02} {}{}: {}",
                addr, f.bit_range.offset, f.bit_range.width, f.name, access, description
            );
            mmap.push(text);
        }
    }
}
