use crate::common::svd_reader;
use crate::common::{str_utils, svd_utils};
use anyhow::{Context, Result};
use std::{fs::File, io::Read, path::Path};
use svd::PeripheralInfo;
use svd_parser::svd::{self, Cluster, Field, Peripheral, Register, RegisterCluster, RegisterInfo};
use svd_rs::FieldInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct CoveredFields {
    all: u32,
    covered: u32,
}

impl core::ops::Add for CoveredFields {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            all: self.all + rhs.all,
            covered: self.covered + rhs.covered,
        }
    }
}

impl core::ops::AddAssign for CoveredFields {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

/// Output sorted text of every peripheral, register, field, and interrupt
/// in the device, such that automated diffing is possible.
pub fn parse_device(svd_file: &Path) -> Result<()> {
    let mut file = File::open(svd_file).expect("svd file doesn't exist");
    match get_text(&mut file) {
        Err(e) => {
            let path_str = svd_file.display();
            Err(e).with_context(|| format!("Parsing {path_str}"))
        }
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
    }
}

fn get_text<R: Read>(svd: &mut R) -> Result<String> {
    let peripherals = svd_reader::peripherals(svd)?;
    Ok(to_text(&peripherals))
}

fn to_text(peripherals: &[Peripheral]) -> String {
    let mut mmap = Vec::new();
    let mut coverage = CoveredFields::default();

    for p in peripherals {
        match p {
            Peripheral::Single(p) => {
                let mut pcov = CoveredFields::default();
                let registers = get_periph_registers(p, peripherals);
                let mut rmmap = Vec::new();
                get_registers(
                    p.base_address,
                    registers.as_ref(),
                    "",
                    &mut rmmap,
                    &mut pcov,
                );
                get_peripheral(
                    p,
                    &mut mmap,
                    if p.derived_from.is_some() {
                        CoveredFields::default()
                    } else {
                        pcov
                    },
                );
                get_interrupts(p, &mut mmap);
                mmap.extend(rmmap);
                coverage += pcov;
            }
            Peripheral::Array(p, d) => {
                let mut pcov = CoveredFields::default();
                for pi in svd::peripheral::expand(p, d) {
                    let registers = get_periph_registers(&pi, peripherals);
                    let mut rmmap = Vec::new();
                    get_registers(
                        pi.base_address,
                        registers.as_ref(),
                        "",
                        &mut rmmap,
                        &mut pcov,
                    );
                    get_peripheral(
                        &pi,
                        &mut mmap,
                        if pi.derived_from.is_some() {
                            CoveredFields::default()
                        } else {
                            pcov
                        },
                    );
                    get_interrupts(&pi, &mut mmap);
                    mmap.extend(rmmap);
                    coverage += pcov;
                }
            }
        }
    }

    mmap.sort();
    mmap.join("\n")
}

fn get_periph_registers<'a>(
    peripheral: &'a PeripheralInfo,
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

fn get_peripheral(peripheral: &PeripheralInfo, mmap: &mut Vec<String>, coverage: CoveredFields) {
    let text = if coverage.all > 0 {
        format!(
            "{} A PERIPHERAL {} ({}/{} fields covered)",
            str_utils::format_address(peripheral.base_address),
            peripheral.name,
            coverage.covered,
            coverage.all,
        )
    } else {
        format!(
            "{} A PERIPHERAL {}",
            str_utils::format_address(peripheral.base_address),
            peripheral.name,
        )
    };
    mmap.push(text);
}

fn get_interrupts(peripheral: &PeripheralInfo, mmap: &mut Vec<String>) {
    for i in &peripheral.interrupt {
        let description = str_utils::get_description(&i.description);
        let text = format!(
            "INTERRUPT {:03}: {} ({}): {description}",
            i.value, i.name, peripheral.name
        );
        mmap.push(text);
    }
}

fn derived_str(dname: &Option<String>) -> String {
    if let Some(dname) = dname.as_ref() {
        format!(" (={dname})")
    } else {
        String::new()
    }
}

fn get_registers(
    base_address: u64,
    registers: Option<&Vec<RegisterCluster>>,
    suffix: &str,
    mmap: &mut Vec<String>,
    coverage: &mut CoveredFields,
) {
    if let Some(registers) = registers {
        for r in registers {
            match &r {
                RegisterCluster::Register(r) => {
                    let mut rcov = CoveredFields::default();
                    let access = svd_utils::access_with_brace(r.properties.access);
                    let derived = derived_str(&r.derived_from);
                    match r {
                        Register::Single(r) => {
                            let addr =
                                str_utils::format_address(base_address + r.address_offset as u64);
                            let rname = r.name.to_string() + suffix;
                            let description = str_utils::get_description(&r.description);
                            let text = format!(
                                "{addr} B  REGISTER {rname}{derived}{access}: {description}"
                            );
                            mmap.push(text);
                            get_fields(r, &addr, mmap, &mut rcov);
                        }
                        Register::Array(r, d) => {
                            for ri in svd::register::expand(r, d) {
                                let addr = str_utils::format_address(
                                    base_address + ri.address_offset as u64,
                                );
                                let rname = &ri.name;
                                let description = str_utils::get_description(&ri.description);
                                let text = format!(
                                    "{addr} B  REGISTER {rname}{derived}{access}: {description}"
                                );
                                mmap.push(text);
                                get_fields(&ri, &addr, mmap, &mut rcov);
                            }
                        }
                    }
                    *coverage += rcov;
                }
                RegisterCluster::Cluster(c) => {
                    let derived = derived_str(&c.derived_from);
                    match c {
                        Cluster::Single(c) => {
                            let caddr = base_address + c.address_offset as u64;
                            let addr = str_utils::format_address(caddr);
                            let cname = &c.name;
                            let description = str_utils::get_description(&c.description);
                            let text = format!("{addr} B  CLUSTER {cname}{derived}: {description}");
                            mmap.push(text);
                            get_registers(caddr, Some(&c.children), "", mmap, coverage);
                        }
                        Cluster::Array(c, d) => {
                            for (ci, idx) in svd::cluster::expand(c, d).zip(d.indexes()) {
                                let caddr = base_address + ci.address_offset as u64;
                                let addr = str_utils::format_address(caddr);
                                let cname = &ci.name;
                                let description = str_utils::get_description(&ci.description);
                                let text =
                                    format!("{addr} B  CLUSTER {cname}{derived}: {description}");
                                mmap.push(text);
                                get_registers(caddr, Some(&c.children), &idx, mmap, coverage);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_fields(
    register: &RegisterInfo,
    addr: &str,
    mmap: &mut Vec<String>,
    coverage: &mut CoveredFields,
) {
    if let Some(fields) = &register.fields {
        for f in fields {
            let derived = derived_str(&f.derived_from);
            let access = svd_utils::access_with_brace(f.access);
            match f {
                Field::Single(f) => {
                    let bit_offset = f.bit_offset();
                    let bit_width = f.bit_width();
                    let fname = &f.name;
                    let description = str_utils::get_description(&f.description);
                    let text = format!(
                        "{addr} C   FIELD {bit_offset:02}w{bit_width:02} {fname}{derived}{access}: {description}"
                    );
                    mmap.push(text);
                    if f.derived_from.is_none() {
                        coverage.all += 1;
                        if is_covered(f) {
                            coverage.covered += 1;
                        }
                    }
                }
                Field::Array(f, d) => {
                    for fi in svd::field::expand(f, d) {
                        let bit_offset = fi.bit_offset();
                        let bit_width = fi.bit_width();
                        let fname = &fi.name;
                        let description = str_utils::get_description(&fi.description);
                        let text = format!(
                            "{addr} C   FIELD {bit_offset:02}w{bit_width:02} {fname}{derived}{access}: {description}"
                        );
                        if fi.derived_from.is_none() {
                            mmap.push(text);
                            coverage.all += 1;
                            if is_covered(&fi) {
                                coverage.covered += 1;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn is_covered(f: &FieldInfo) -> bool {
    !f.enumerated_values.is_empty() || f.write_constraint.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    static SVD: &str = r##"
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
                            <writeConstraint>
                                <range>
                                    <minimum>0</minimum>
                                    <maximum>0x3</maximum>
                                </range>
                            </writeConstraint>
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
                    <fields>
                    <field>
                        <name>F3</name>
                        <description>Field 3</description>
                        <bitOffset>10</bitOffset>
                        <bitWidth>1</bitWidth>
                        <enumeratedValues>
                            <name>EV_NAME</name>
                            <enumeratedValue>
                                <name>VAL1</name>
                                <description>Value description 1</description>
                                <value>0</value>
                            </enumeratedValue>
                            <enumeratedValue>
                                <name>VAL2</name>
                                <description>Value description 2</description>
                                <value>1</value>
                            </enumeratedValue>
                        </enumeratedValues>
                    </field>
                </fields>
                </register>
            </registers>
        </peripheral>
        <peripheral derivedFrom="PeriphB">
            <name>PeriphC</name>
            <description>Peripheral C</description>
            <baseAddress>0x10020000</baseAddress>
        </peripheral>
    </peripherals>
</device>"##;

    static EXPECTED_MMAP: &str = r"0x10000000 A PERIPHERAL PeriphA (1/2 fields covered)
0x10000010 B  REGISTER REG1: Register A1
0x10000010 C   FIELD 05w02 F1: Field 1
0x10000010 C   FIELD 10w01 F2: Field 2
0x10000014 B  REGISTER REG2: Register A2
0x10010000 A PERIPHERAL PeriphB (1/1 fields covered)
0x10010010 B  REGISTER REG1: Register B1
0x10010010 C   FIELD 10w01 F3: Field 3
0x10020000 A PERIPHERAL PeriphC
0x10020010 B  REGISTER REG1: Register B1
0x10020010 C   FIELD 10w01 F3: Field 3
INTERRUPT 001: INT_A1 (PeriphA): Interrupt A1
INTERRUPT 002: INT_B2 (PeriphB): Interrupt B2";

    #[test]
    fn mmap() {
        let mut svd = SVD.as_bytes();
        let actual_mmap = get_text(&mut svd).unwrap();
        assert_eq!(EXPECTED_MMAP, actual_mmap);
    }
}
