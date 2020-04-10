use crate::patch::yaml_parser::{self, Merge, PeripheralNode, YamlBody};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
        let path_dir = path.parent().unwrap();
        let child_included_yamls = yaml_includes(&mut child, &path_dir);
        included.extend(child_included_yamls);
        parent.merge(&child);
    }
    included
}
