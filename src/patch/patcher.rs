use crate::patch::yaml::yaml_parser as yml;
use svd_parser::svd::Device;
use yml::YamlBody;

#[derive(Debug)]
pub struct Patcher {
    pub svd: Device,
    pub yaml: YamlBody, // device
}
