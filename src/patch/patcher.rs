use crate::patch::modify;
use crate::{common::svd_reader, patch::yaml::yaml_parser::YamlBody};
use std::path::Path;
use svd::Device;
use svd_parser as svd;

pub struct Patcher {
    pub svd: Device,
    pub yaml: YamlBody, // device
}

impl Patcher {
    pub fn process_device(&mut self) {
        self.delete_peripherals();
        self.copy_peripherals();
        self.modify_device();
    }

    fn delete_peripherals(&mut self) {
        let delete = &self.yaml.commands.delete;
        // delete all peripherals contained in delete
        self.svd.peripherals.retain(|p| !delete.contains(&p.name));
    }

    fn copy_peripherals(&mut self) {
        let copy = &self.yaml.commands.copy;
        for (dest, src) in copy {
            let src = &src.from;
            let src: Vec<&str> = src.split(':').collect();
            let src_peripheral = match src.len() {
                1 => get_peripheral_copy(&self.svd, src[0]),
                2 => {
                    let svd_path = Path::new(&src[0]);
                    // TODO add yaml path here
                    let svd = svd_reader::device(svd_path);
                    get_peripheral_copy(&svd, src[1])
                }
                _ => panic!("_copy - from has too many ':'"),
            };
            let mut src_peripheral = match src_peripheral {
                None => panic!("peripheral {} not found", src.last().unwrap()),
                Some(periph) => periph,
            };
            src_peripheral.name = dest.clone();
            let dest_periph = get_peripheral_copy(&self.svd, dest);
            match dest_periph {
                Some(dest_periph) => {
                    src_peripheral.base_address = dest_periph.base_address;
                    src_peripheral.interrupt = dest_periph.interrupt;
                    self.svd
                        .peripherals
                        .retain(|p| p.name != src_peripheral.name);
                }
                None => {
                    src_peripheral.interrupt = vec![];
                }
            }
            self.svd.peripherals.push(src_peripheral);
        }
    }

    fn modify_device(&mut self) {
        if let Some(modify) = &self.yaml.commands.modify {
            if let Some(new_cpu) = &modify.cpu {
                modify::modify_cpu(&mut self.svd.cpu, new_cpu);
            }
            for (periph_name, new_periph) in &modify.peripherals {
                // TODO At the moment we ignore addressBlocks feature since it is
                //      never used in the stm32-rs repository. Is it ok?
                let mut old_periph = get_peripheral_mut(&mut self.svd, periph_name)
                    .expect("peripheral {} of _modify not found in svd");
                new_periph.modify(&mut old_periph);
            }
        }
    }
}

fn get_peripheral_mut<'a>(
    svd: &'a mut Device,
    peripheral_name: &str,
) -> Option<&'a mut svd::Peripheral> {
    svd.peripherals
        .iter_mut()
        .filter(|p| p.name == peripheral_name)
        .next()
}

fn get_peripheral_copy(svd: &Device, peripheral_name: &str) -> Option<svd::Peripheral> {
    svd.peripherals
        .iter()
        .filter(|p| p.name == peripheral_name)
        .next()
        .map(|p| p.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::path::Path;

    #[test]
    fn delete_peripherals() {
        let mut patcher = test_utils::get_patcher(Path::new("delete"));
        assert_eq!(patcher.svd.peripherals.len(), 3);
        patcher.delete_peripherals();
        assert_eq!(patcher.svd.peripherals.len(), 1);
        let remaining_periph = &patcher.svd.peripherals[0];
        assert_eq!(remaining_periph.name, "DAC2");
    }

    #[test]
    fn copy_peripherals() {
        let mut patcher = test_utils::get_patcher(Path::new("copy"));
        assert_eq!(patcher.svd.peripherals.len(), 3);
        let dac1 = get_peripheral_copy(&patcher.svd, "DAC1").unwrap();
        let dac2 = get_peripheral_copy(&patcher.svd, "DAC2").unwrap();
        assert_ne!(dac1.registers, dac2.registers);

        patcher.copy_peripherals();
        assert_eq!(patcher.svd.peripherals.len(), 3);

        let dac2 = get_peripheral_copy(&patcher.svd, "DAC2").unwrap();
        assert_eq!(dac1.registers, dac2.registers);
    }

    #[test]
    fn modify_device() {
        let mut patcher = test_utils::get_patcher(Path::new("modify"));

        // check cpu initial config
        let cpu = &patcher.svd.cpu.clone().unwrap();
        assert_eq!(cpu.nvic_priority_bits, 3);

        // check peripheral initial config
        assert_eq!(patcher.svd.peripherals.len(), 2);
        let dac1 = get_peripheral_copy(&patcher.svd, "DAC1").unwrap();
        assert_eq!(dac1.name, "DAC1");
        assert_eq!(dac1.description, None);

        patcher.modify_device();

        // check cpu final config
        let cpu = &patcher.svd.cpu.clone().unwrap();
        assert_eq!(cpu.nvic_priority_bits, 4);

        // check peripheral final config
        let dac1 = get_peripheral_copy(&patcher.svd, "DAC11").unwrap();
        assert_eq!(dac1.name, "DAC11");
        assert_eq!(
            dac1.description,
            Some("Digital-to-analog converter".to_string())
        );
    }
}
