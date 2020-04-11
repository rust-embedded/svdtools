use crate::patch::yaml_parser::YamlBody;
use svd_parser::Device;

pub struct Patcher {
    pub svd: Device,
    pub yaml: YamlBody, // device
}

impl Patcher {
    pub fn process_device(&mut self) {
        self.delete_peripherals();
        self.copy_peripherals();
    }

    fn delete_peripherals(&mut self) {
        let delete = &self.yaml.commands.delete;
        // delete all peripherals contained in delete
        self.svd.peripherals.retain(|p| !delete.contains(&p.name));
    }

    fn copy_peripherals(&mut self) {}
}

#[cfg(test)]
mod tests {
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
}
