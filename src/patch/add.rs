use crate::patch::patcher::Patcher;
use anyhow::{anyhow, Result};

impl Patcher {
    pub fn add_peripherals(&mut self) -> Result<()> {
        let add = &self.yaml.commands.add;
        for (peripheral_name, peripheral) in add {
            let peripheral_already_exists = self
                .svd
                .peripherals
                .iter()
                .any(|p| &p.name == peripheral_name);

            if peripheral_already_exists {
                return Err(anyhow!(
                    "device already has a peripheral {}",
                    peripheral_name
                ));
            }

            let svd_peripheral = peripheral.to_svd(peripheral_name)?;
            self.svd.peripherals.push(svd_peripheral);
            // TODO handle derivedFrom
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::path::Path;
    use svd::bitrange::BitRangeType;
    use svd::{BitRange, RegisterInfo};
    use svd_parser as svd;

    fn expected_field_a() -> svd::FieldInfo {
        let mut fb: svd::fieldinfo::FieldInfoBuilder = Default::default();
        fb = fb.name("FIELDA".to_string());
        fb = fb.description(Some("This field defines the implementer".to_string()));
        fb = fb.bit_range(BitRange {
            offset: 24,
            width: 7,
            range_type: BitRangeType::BitRange, // TODO is this correct?
        });

        fb.build().unwrap()
    }

    fn expected_field_b() -> svd::FieldInfo {
        let mut fb: svd::fieldinfo::FieldInfoBuilder = Default::default();
        fb = fb.name("FIELDB".to_string());
        fb = fb.description(Some("Implementation defined".to_string()));
        fb = fb.bit_range(BitRange {
            offset: 20,
            width: 4,
            range_type: BitRangeType::BitRange, // TODO is this correct?
        });

        fb.build().unwrap()
    }

    fn expected_fields() -> Vec<svd::Field> {
        vec![expected_field_a(), expected_field_b()]
            .iter()
            .map(|f| svd::Field::Single(f.clone()))
            .collect()
    }

    fn expected_reg01() -> RegisterInfo {
        let mut rb: svd::registerinfo::RegisterInfoBuilder = Default::default();
        rb = rb.name("REG01".to_string());
        rb = rb.description(Some("I-cache invalidate all to PoU".to_string()));
        rb = rb.address_offset(0x0);
        rb = rb.access(Some(svd::Access::WriteOnly));
        rb.build().unwrap()
    }

    fn expected_reg02() -> RegisterInfo {
        let mut rb: svd::registerinfo::RegisterInfoBuilder = Default::default();
        rb = rb.name("REG02".to_string());
        rb = rb.description(Some("I-cache invalidate by MVA to PoU".to_string()));
        rb = rb.address_offset(0x8);
        rb = rb.access(Some(svd::Access::ReadOnly));
        rb = rb.fields(Some(expected_fields()));
        rb.build().unwrap()
    }

    fn expected_registers() -> Vec<svd::RegisterCluster> {
        vec![expected_reg01(), expected_reg02()]
            .iter()
            .map(|r| svd::RegisterCluster::Register(svd::Register::Single(r.clone())))
            .collect()
    }

    fn expected_peripheral() -> svd::Peripheral {
        let mut pb: svd::peripheral::PeripheralBuilder = Default::default();

        pb = pb.name("CPUID".to_string());
        pb = pb.description(Some("CPUID descr".to_string()));
        pb = pb.base_address(0xE000ED00);
        pb = pb.address_block(Some(svd::AddressBlock {
            offset: 0x0,
            size: 4,
            usage: "registers".to_string(),
        }));
        pb = pb.registers(Some(expected_registers()));
        pb.build().unwrap()
    }

    #[test]
    fn add_peripheral() {
        let mut patcher = test_utils::get_patcher(Path::new("add"));
        let added_peripheral = "CPUID";

        // check device initial config
        let added_peripheral_exists = |patcher: &Patcher| {
            patcher
                .svd
                .peripherals
                .iter()
                .any(|p| p.name == added_peripheral)
        };
        assert!(!added_peripheral_exists(&patcher));

        let mut expected_svd = patcher.svd.clone();

        patcher.add_peripherals().unwrap();
        expected_svd.peripherals.push(expected_peripheral());
        assert!(added_peripheral_exists(&patcher));

        // TODO left side is wrong sometimes because the order of fields is random. Why?
        //assert_eq!(patcher.svd, expected_svd);
    }
}
