use crate::common::svd_reader;
use crate::common::{str_utils, svd_utils};
use anyhow::Result;
use std::{fs::File, io::Read, path::Path};
use svd_parser::svd::{Cluster, Field, Peripheral, Register, RegisterCluster, RegisterInfo};

/// Output sorted text of every peripheral, register, field, and interrupt
/// in the device, such that automated diffing is possible.
pub fn parse_device(svd_file: &Path) {
    let mut file = File::open(svd_file).expect("svd file doesn't exist");
    match get_text(&mut file) {
        Err(e) => {
            let path_str = svd_file.display();
            eprintln!("cannot parse {}: {}", path_str, e.to_string());
        }
        Ok(text) => println!("{}", text),
    }
}

fn get_text<R: Read>(svd: &mut R) -> Result<String> {
    let peripherals = svd_reader::peripherals(svd)?;
    Ok(to_text(&peripherals))
}

fn to_text(peripherals: &[Peripheral]) -> String {
    let mut mmap: Vec<String> = vec![];

    for p in peripherals {
        get_peripheral(&p, &mut mmap);
        get_interrupts(&p, &mut mmap);
        let registers = get_periph_registers(p, peripherals);
        get_registers(p.base_address, registers.as_ref(), "", &mut mmap);
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
                    break;
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
    base_address: u64,
    registers: Option<&Vec<RegisterCluster>>,
    suffix: &str,
    mmap: &mut Vec<String>,
) {
    if let Some(registers) = registers {
        for r in registers {
            match &r {
                RegisterCluster::Register(r) => {
                    let description = str_utils::get_description(&r.description);
                    let access = svd_utils::access_with_brace(r.properties.access);
                    let first_addr = base_address + r.address_offset as u64;
                    match r {
                        Register::Single(r) => {
                            let addr = str_utils::format_address(first_addr);
                            let rname = r.name.to_string() + suffix;
                            let text = format!(
                                "{} B  REGISTER {}{}: {}",
                                addr, rname, access, description
                            );
                            mmap.push(text);
                            get_fields(r, &addr, mmap);
                        }
                        Register::Array(r, d) => {
                            for (i, idx) in d.indexes().enumerate() {
                                let addr = str_utils::format_address(
                                    first_addr + (i as u64) * (d.dim_increment as u64),
                                );
                                let rname = r.name.replace("%s", &idx);
                                let description = description.replace("%s", &idx);
                                let text = format!(
                                    "{} B  REGISTER {}{}: {}",
                                    addr, rname, access, description
                                );
                                mmap.push(text);
                                get_fields(r, &addr, mmap);
                            }
                        }
                    }
                }
                RegisterCluster::Cluster(c) => {
                    let description = str_utils::get_description(&c.description);
                    let first_addr = base_address + c.address_offset as u64;
                    match c {
                        Cluster::Single(c) => {
                            let addr = str_utils::format_address(first_addr);
                            let text = format!("{} B  CLUSTER {}: {}", addr, c.name, description);
                            mmap.push(text);
                            get_registers(first_addr, Some(&c.children), "", mmap);
                        }
                        Cluster::Array(c, d) => {
                            for (i, idx) in d.indexes().enumerate() {
                                let caddr = first_addr + (i as u64) * (d.dim_increment as u64);
                                let addr = str_utils::format_address(caddr);
                                let cname = c.name.replace("%s", &idx);
                                let description = description.replace("%s", &idx);
                                let text =
                                    format!("{} B  CLUSTER {}: {}", addr, cname, description);
                                mmap.push(text);
                                get_registers(caddr, Some(&c.children), &idx, mmap);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_fields(register: &RegisterInfo, addr: &str, mmap: &mut Vec<String>) {
    if let Some(fields) = &register.fields {
        for f in fields {
            let description = str_utils::get_description(&f.description);
            let access = svd_utils::access_with_brace(f.access);
            match f {
                Field::Single(f) => {
                    let bit_offset = f.bit_range.offset;
                    let text = format!(
                        "{} C   FIELD {:02}w{:02} {}{}: {}",
                        addr, bit_offset, f.bit_range.width, f.name, access, description
                    );
                    mmap.push(text);
                }
                Field::Array(f, d) => {
                    for (i, idx) in d.indexes().enumerate() {
                        let bit_offset = f.bit_range.offset + (i as u32) * d.dim_increment;
                        let fname = f.name.replace("%s", &idx);
                        let text = format!(
                            "{} C   FIELD {:02}w{:02} {}{}: {}",
                            addr, bit_offset, f.bit_range.width, fname, access, description
                        );
                        mmap.push(text);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SVD: &str = r"
<device>
    <name>dev</name>
    <peripherals>
        <peripheral>
            <name>PeriphA</name>
            <description>Peripheral A</description>
            <baseAddress>0x10000000</baseAddress>
            <interrupt>
                <name>INT_A1</name>
                <description>Interrupt A1</description>
                <value>1</value>
            </interrupt>
            <registers>
                <register>
                    <name>REG1</name>
                    <addressOffset>0x10</addressOffset>
                    <description>Register A1</description>
                    <fields>
                        <field>
                            <name>F1</name>
                            <description>Field 1</description>
                            <bitOffset>5</bitOffset>
                            <bitWidth>2</bitWidth>
                        </field>
                        <field>
                            <name>F2</name>
                            <description>Field 2</description>
                            <bitOffset>10</bitOffset>
                            <bitWidth>1</bitWidth>
                        </field>
                    </fields>
                </register>
                <register>
                    <name>REG2</name>
                    <addressOffset>0x14</addressOffset>
                    <description>Register A2</description>
                </register>
            </registers>
        </peripheral>
        <peripheral>
            <name>PeriphB</name>
            <description>Peripheral B</description>
            <baseAddress>0x10010000</baseAddress>
            <interrupt>
                <name>INT_B2</name>
                <description>Interrupt B2</description>
                <value>2</value>
            </interrupt>
            <registers>
                <register>
                    <name>REG1</name>
                    <addressOffset>0x10</addressOffset>
                    <description>Register B1</description>
                </register>
            </registers>
        </peripheral>
    </peripherals>
</device>";

    static EXPECTED_MMAP: &str = r"0x10000000 A PERIPHERAL PeriphA
0x10000010 B  REGISTER REG1: Register A1
0x10000010 C   FIELD 05w02 F1: Field 1
0x10000010 C   FIELD 10w01 F2: Field 2
0x10000014 B  REGISTER REG2: Register A2
0x10010000 A PERIPHERAL PeriphB
0x10010010 B  REGISTER REG1: Register B1
INTERRUPT 001: INT_A1 (PeriphA): Interrupt A1
INTERRUPT 002: INT_B2 (PeriphB): Interrupt B2";

    #[test]
    fn mmap() {
        let mut svd = SVD.as_bytes();
        let actual_mmap = get_text(&mut svd).unwrap();
        assert_eq!(EXPECTED_MMAP, actual_mmap);
    }
}
