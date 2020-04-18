use crate::common::svd_reader;
use crate::patch::include;
use crate::patch::patcher::Patcher;
use crate::patch::yaml::yaml_parser;
use anyhow::Result;
use std::io::Write;
use std::{fs::File, path::Path};
use svd_parser;
use yaml_parser::YamlRoot;

pub fn patch(yaml_file: &Path) -> Result<()> {
    let mut yaml: YamlRoot = yaml_parser::from_path(yaml_file);

    let yaml_dir = yaml_file.parent().expect("wrong yaml file path");

    let svdpath = yaml_dir.join(&yaml.svd);

    let svd = svd_reader::device(&svdpath)?;

    let yaml_dir = yaml_file.parent().unwrap();
    include::yaml_includes(&mut yaml.body, yaml_dir);

    let mut patcher = Patcher {
        svd,
        yaml: yaml.body,
    };
    patcher.process_device()?;

    let xml_out = svd_parser::encode(&patcher.svd).unwrap();

    let svdpath_out = svdpath.with_extension("svd.patched");
    let mut out_file = File::create(svdpath_out).unwrap();
    out_file.write_all(xml_out.as_bytes()).unwrap();
    Ok(())
}
