use svd_parser::svd::{Cpu, Device, Endian, Peripheral, PeripheralInfo};
use yaml_rust::yaml::Hash;

use std::{fs::File, io::Read, path::Path};

use super::make_peripheral;
use super::modify_register_properties;
use super::peripheral::PeripheralExt;
use super::yaml_ext::{parse_i64, GetVal};
use super::{abspath, matchname, VAL_LVL};

pub struct PerIter<'a, 'b> {
    it: std::slice::IterMut<'a, Peripheral>,
    spec: &'b str,
    check_derived: bool,
}

impl<'a, 'b> Iterator for PerIter<'a, 'b> {
    type Item = &'a mut Peripheral;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.it.next() {
            if matchname(&next.name, self.spec)
                && !(self.check_derived && next.derived_from.is_some())
            {
                return Some(next);
            }
        }
        None
    }
}

/// Collecting methods for processing device contents
pub trait DeviceExt {
    /// Iterates over all peripherals that match pspec
    fn iter_peripherals<'a, 'b>(
        &'a mut self,
        spec: &'b str,
        check_derived: bool,
    ) -> PerIter<'a, 'b>;

    /// Work through a device, handling all peripherals
    fn process(&mut self, device: &Hash, update_fields: bool);

    /// Delete registers matched by rspec inside ptag
    fn delete_peripheral(&mut self, pspec: &str);

    /// Create copy of peripheral
    fn copy_peripheral(&mut self, pname: &str, pmod: &Hash, path: &Path);

    /// Modify the `cpu` node inside `device` according to `mod`
    fn modify_cpu(&mut self, cmod: &Hash);

    /// Modify pspec inside device according to pmod
    fn modify_peripheral(&mut self, pspec: &str, pmod: &Hash);

    /// Add pname given by padd to device
    fn add_peripheral(&mut self, pname: &str, padd: &Hash);

    /// Remove registers from pname and mark it as derivedFrom pderive.
    /// Update all derivedFrom referencing pname
    fn derive_peripheral(&mut self, pname: &str, pderive: &str);

    /// Move registers from pold to pnew.
    /// Update all derivedFrom referencing pold
    fn rebase_peripheral(&mut self, pnew: &str, pold: &str);

    /// Work through a peripheral, handling all registers
    fn process_peripheral(&mut self, pspec: &str, peripheral: &Hash, update_fields: bool);
}

impl DeviceExt for Device {
    fn iter_peripherals<'a, 'b>(
        &'a mut self,
        spec: &'b str,
        check_derived: bool,
    ) -> PerIter<'a, 'b> {
        // check_derived=True
        PerIter {
            spec,
            check_derived,
            it: self.peripherals.iter_mut(),
        }
    }

    fn process(&mut self, device: &Hash, update_fields: bool) {
        // Handle any deletions
        for pspec in device.str_vec_iter("_delete") {
            self.delete_peripheral(pspec);
        }

        // Handle any copied peripherals
        for (pname, val) in device.hash_iter("_copy") {
            self.copy_peripheral(
                pname.as_str().unwrap(),
                val.as_hash().unwrap(),
                Path::new(device.get_str("_path").unwrap()),
            );
        }

        // Handle any modifications
        for (key, val) in device.hash_iter("_modify") {
            let key = key.as_str().unwrap();
            match key {
                "cpu" => self.modify_cpu(val.as_hash().unwrap()),
                "_peripherals" => {
                    for (pspec, pmod) in val.as_hash().unwrap() {
                        self.modify_peripheral(pspec.as_str().unwrap(), pmod.as_hash().unwrap())
                    }
                }
                "vendor" => {
                    todo!()
                }
                "vendorID" => {
                    todo!()
                }
                "name" => self.name = val.as_str().unwrap().into(),
                "series" => {
                    todo!()
                }
                "version" => self.version = val.as_str().map(String::from),
                "description" => self.description = val.as_str().map(String::from),
                "licenseText" => {
                    todo!()
                }
                "headerSystemFilename" => {
                    todo!()
                }
                "headerDefinitionsPrefix" => {
                    todo!()
                }
                "addressUnitBits" => self.address_unit_bits = parse_i64(val).map(|v| v as u32),
                "width" => self.width = parse_i64(val).map(|v| v as u32),
                "size" | "access" | "protection" | "resetValue" | "resetMask" => {
                    modify_register_properties(&mut self.default_register_properties, key, val)
                }

                _ => self.modify_peripheral(key, val.as_hash().unwrap()),
            }
        }

        // Handle any new peripherals (!)
        for (pname, padd) in device.hash_iter("_add") {
            self.add_peripheral(pname.as_str().unwrap(), padd.as_hash().unwrap());
        }

        // Handle any derived peripherals
        for (pname, pderive) in device.hash_iter("_derive") {
            self.derive_peripheral(pname.as_str().unwrap(), pderive.as_str().unwrap());
        }

        // Handle any rebased peripherals
        for (pname, pold) in device.hash_iter("_rebase") {
            self.rebase_peripheral(pname.as_str().unwrap(), pold.as_str().unwrap());
        }

        // Now process all peripherals
        for (periphspec, val) in device {
            let periphspec = periphspec.as_str().unwrap();
            if !periphspec.starts_with("_") {
                //val["_path"] = device["_path"]; // TODO: check
                self.process_peripheral(periphspec, val.as_hash().unwrap(), update_fields)
            }
        }
    }

    fn delete_peripheral(&mut self, pspec: &str) {
        self.peripherals.retain(|p| !(matchname(&p.name, pspec)));
    }

    fn copy_peripheral(&mut self, pname: &str, pmod: &Hash, path: &Path) {
        let pcopysrc = pmod.get_str("from").unwrap().split(":").collect::<Vec<_>>();
        let mut new = match pcopysrc.as_slice() {
            [ppath, pcopyname] => {
                let f = File::open(abspath(path, &Path::new(ppath))).unwrap();
                let mut contents = String::new();
                (&f).read_to_string(&mut contents).unwrap();
                let filedev = svd_parser::parse(&contents).expect("Failed to parse input SVD");
                filedev
                    .peripherals
                    .iter()
                    .find(|p| &p.name == pcopyname)
                    .unwrap_or_else(|| panic!("peripheral {} not found", pcopyname))
                    .clone()
            }
            [pcopyname] => {
                let mut new = self
                    .peripherals
                    .iter()
                    .find(|p| &p.name == pcopyname)
                    .unwrap_or_else(|| panic!("peripheral {} not found", pcopyname))
                    .clone();
                // When copying from a peripheral in the same file, remove any interrupts.
                new.interrupt = Vec::new();
                new
            }
            _ => panic!(),
        };
        new.name = pname.into();
        if let Some(ptag) = self.peripherals.iter_mut().find(|p| &p.name == pname) {
            new.base_address = ptag.base_address;
            new.interrupt = std::mem::take(&mut ptag.interrupt);
            *ptag = new;
        } else {
            self.peripherals.push(new)
        }
    }

    fn modify_cpu(&mut self, cmod: &Hash) {
        let mut cpu = Cpu::builder();
        if let Some(name) = cmod.get_str("name") {
            cpu = cpu.name(name.into());
        }
        if let Some(revision) = cmod.get_str("revision") {
            cpu = cpu.revision(revision.into());
        }
        if let Some(endian) = cmod.get_str("endian").and_then(Endian::parse_str) {
            cpu = cpu.endian(endian);
        }
        if let Some(mpu_present) = cmod.get_bool("mpuPresent") {
            cpu = cpu.mpu_present(mpu_present);
        }
        if let Some(fpu_present) = cmod.get_bool("fpuPresent") {
            cpu = cpu.fpu_present(fpu_present);
        }
        if let Some(nvic_priority_bits) = cmod.get_i64("nvicPrioBits") {
            cpu = cpu.nvic_priority_bits(nvic_priority_bits as u32);
        }
        if let Some(has_vendor_systick) = cmod.get_bool("vendorSystickConfig") {
            cpu = cpu.has_vendor_systick(has_vendor_systick);
        }
        if let Some(c) = self.cpu.as_mut() {
            c.modify_from(cpu, VAL_LVL).unwrap();
        } else {
            self.cpu = Some(cpu.build(VAL_LVL).unwrap());
        }
    }

    fn modify_peripheral(&mut self, pspec: &str, pmod: &Hash) {
        for ptag in self.iter_peripherals(pspec, true) {
            ptag.modify_from(make_peripheral(pmod), VAL_LVL).unwrap();
        }
    }

    fn add_peripheral(&mut self, pname: &str, padd: &Hash) {
        if self.peripherals.iter().find(|p| p.name == pname).is_some() {
            panic!("device already has a peripheral {}", pname);
        }

        self.peripherals.push(
            make_peripheral(padd)
                .name(pname.to_string())
                .build(VAL_LVL)
                .unwrap()
                .single(),
        );
    }

    fn derive_peripheral(&mut self, pname: &str, pderive: &str) {
        assert!(
            self.peripherals
                .iter()
                .find(|p| &p.name == pderive)
                .is_some(),
            "peripheral {} not found",
            pderive
        );
        self.peripherals
            .iter_mut()
            .find(|p| &p.name == pname)
            .unwrap_or_else(|| panic!("peripheral {} not found", pname))
            .modify_from(
                PeripheralInfo::builder().derived_from(Some(pderive.into())),
                VAL_LVL,
            )
            .unwrap();
        for p in self
            .peripherals
            .iter_mut()
            .filter(|p| p.derived_from.as_deref() == Some(pname))
        {
            p.derived_from = Some(pderive.into());
        }
    }

    fn rebase_peripheral(&mut self, pnew: &str, pold: &str) {
        let old = self
            .peripherals
            .iter_mut()
            .find(|p| &p.name == pold)
            .unwrap_or_else(|| panic!("peripheral {} not found", pold));
        let mut d = std::mem::replace(
            old,
            PeripheralInfo::builder()
                .name(pold.into())
                .base_address(old.base_address)
                .interrupt(if old.interrupt.is_empty() {
                    None
                } else {
                    Some(old.interrupt.clone())
                })
                .derived_from(Some(pnew.into()))
                .build(VAL_LVL)
                .unwrap()
                .single(),
        );
        let new = self
            .peripherals
            .iter_mut()
            .find(|p| &p.name == pnew)
            .unwrap_or_else(|| panic!("peripheral {} not found", pnew));
        d.name = new.name.clone();
        d.base_address = new.base_address;
        d.interrupt = new.interrupt.clone();
        *new = d;
        for p in self
            .peripherals
            .iter_mut()
            .filter(|p| p.derived_from.as_deref() == Some(pold))
        {
            p.derived_from = Some(pnew.into());
        }
    }

    fn process_peripheral(&mut self, pspec: &str, peripheral: &Hash, update_fields: bool) {
        // Find all peripherals that match the spec
        let mut pcount = 0;
        for ptag in self.iter_peripherals(pspec, false) {
            pcount += 1;
            ptag.process(peripheral, update_fields);
        }
        if pcount == 0 {
            panic!("Could not find {}", pspec);
        }
    }
}
