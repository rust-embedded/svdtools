use crate::patch::yaml_parser::YamlBody;
use svd_parser::Device;

pub struct Patcher {
    pub svd: Device,
    pub yaml: YamlBody, // device
}

impl Patcher {
    pub fn process_device(&mut self) {
        self.delete_peripherals();
    }

    fn delete_peripherals(&mut self) {
        let delete = &self.yaml.commands.delete;
        // delete all peripherals contained in delete
        self.svd.peripherals.retain(|p| !delete.contains(&p.name));
    }
}
