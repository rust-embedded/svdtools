use crate::common::svd_reader;
use crate::patch::include;
use crate::patch::patcher::Patcher;
use crate::patch::yaml::yaml_parser;
use std::path::Path;
use yaml_parser::YamlRoot;

pub fn patch(yaml_file: &Path) {
    let mut yaml: YamlRoot = yaml_parser::from_path(yaml_file);
    println!("root: {:#?}", yaml);

    let yaml_dir = yaml_file.parent().expect("wrong yaml file path");

    let svdpath = yaml_dir.join(&yaml.svd);
    println!("svdpath: {:?}", svdpath);

    let _svdpath_out = svdpath.join(Path::new(".patched"));

    let svd = svd_reader::device(&svdpath);

    let yaml_dir = yaml_file.parent().unwrap();
    include::yaml_includes(&mut yaml.body, yaml_dir);

    let mut patcher = Patcher {
        svd,
        yaml: yaml.body,
    };
    patcher.process_device();
}
