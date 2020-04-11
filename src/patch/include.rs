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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::yaml_parser::Field;
    use crate::{patch::yaml_parser::YamlRoot, test_utils};

    #[test]
    fn all_yamls_are_included() {
        let test_dir = test_utils::res_dir().join(Path::new("include"));
        let yaml_file = test_dir.join(Path::new("patch.yaml"));
        let mut yaml: YamlRoot = yaml_parser::from_path(&yaml_file);
        let actual_includes = yaml_includes(&mut yaml.body, &test_dir);

        let subdir = test_dir.join(Path::new("subdir"));
        let expected_includes = vec![subdir.join("tsc.yaml"), subdir.join("other.yaml")];
        assert_eq!(actual_includes, expected_includes);

        let dac1_periph = yaml.body.peripherals.get("DAC1").unwrap();
        let cr_reg = dac1_periph.registers.get("CR").unwrap();
        let en1_field = cr_reg.commands.modify.get("EN1").unwrap();
        let expected_field = Field {
            name: None,
            description: Some("EN2 description".to_string()),
            bit_offset: Some(2),
            bit_width: Some(4),
        };
        assert_eq!(en1_field, &expected_field);
    }
}
