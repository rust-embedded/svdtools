use crate::common::svd_reader;
use crate::patch::yaml_parser;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
use yaml_parser::{PeripheralNode, YamlBody, YamlRoot};

fn get_abs_paths(parent_dir: &Path, relative_paths: &[PathBuf]) -> Vec<PathBuf> {
    relative_paths
        .iter()
        .map(|i| {
            let path = parent_dir.join(&i);
            fs::canonicalize(path).expect("invalid include")
        })
        .collect()
}

fn yaml_peripheral_includes(parent: &mut PeripheralNode, parent_dir: &Path) -> Vec<PathBuf> {
    let mut included: Vec<PathBuf> = vec![];
    let paths: Vec<PathBuf> = get_abs_paths(parent_dir, &parent.commands.include);
    for path in paths {
        if included.contains(&path) {
            continue;
        }

        let mut child: PeripheralNode = yaml_parser::from_path(&path);
        included.push(path.clone());

        // Process any top-level includes in child

        let path_dir = path.parent().unwrap();
        let child_included_yamls = yaml_peripheral_includes(&mut child, &path_dir);
        included.extend(child_included_yamls);
        parent.merge(&child);
    }
    included
}

pub fn yaml_includes(parent: &mut YamlBody, parent_dir: &Path) -> Vec<PathBuf> {
    let mut included: Vec<PathBuf> = vec![];
    let paths: Vec<PathBuf> = get_abs_paths(parent_dir, &parent.commands.include);
    for path in paths {
        if included.contains(&path) {
            continue;
        }

        let mut child: YamlBody = yaml_parser::from_path(&path);
        included.push(path.clone());

        // Process any peripheral-level includes in child
        for mut pspec in &mut child.peripherals {
            let path_dir = path.parent().unwrap();
            let child_included = yaml_peripheral_includes(&mut pspec.1, &path_dir);
            included.extend(child_included);
        }

        // Process any top-level includes in child
        let child_included_yamls = yaml_includes(&mut child, &path);
        included.extend(child_included_yamls);
        parent.merge(&child);
    }
    included
}

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
    yaml_includes(&mut yaml.body, yaml_dir);
}
