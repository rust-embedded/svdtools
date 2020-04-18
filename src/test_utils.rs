use crate::{
    common::svd_reader,
    patch::{
        patcher::Patcher,
        yaml::yaml_parser::{self, YamlRoot},
    },
};
use std::path::{Path, PathBuf};

pub fn res_dir() -> PathBuf {
    std::env::current_dir().unwrap().join(Path::new("res"))
}

pub fn get_patcher(test_subdir: &Path) -> Patcher {
    let test_subdir = res_dir().join(test_subdir);
    let yaml_file = test_subdir.join("patch.yaml");
    let yaml: YamlRoot = yaml_parser::from_path(&yaml_file);

    let svdpath = test_subdir.join(&yaml.svd);
    let svd = svd_reader::device(&svdpath).unwrap();

    Patcher {
        svd,
        yaml: yaml.body,
    }
}
