use crate::common::svd_reader;
use crate::patch::include;
use crate::patch::yaml_parser;
use std::{fs::File, path::Path};
use yaml_parser::YamlRoot;

pub fn patch(yaml_file: &Path) {
    let mut yaml: YamlRoot = yaml_parser::from_path(yaml_file);
    println!("root: {:#?}", yaml);

    // let mut m = yaml.body;

    let yaml_dir = yaml_file.parent().expect("wrong yaml file path");

    let svdpath = yaml_dir.join(&yaml.svd);
    println!("svdpath: {:?}", svdpath);

    let _svdpath_out = svdpath.join(Path::new(".patched"));

    let mut svd_file = File::open(svdpath).expect("svd file doesn't exist");
    let _peripherals = svd_reader::peripherals(&mut svd_file);

    let yaml_dir = yaml_file.parent().unwrap();
    include::yaml_includes(&mut yaml.body, yaml_dir);
}
