use crate::patch::yaml::yaml_parser as yml;
use svd::Device;
use svd_parser as svd;
use yml::YamlBody;

#[derive(Debug)]
pub struct Patcher {
    pub svd: Device,
    pub yaml: YamlBody, // device
}
