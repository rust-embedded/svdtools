use crate::patch::yaml_includes;
use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use yaml_rust::{Yaml, YamlLoader};

fn write_to_file(file: &mut File, string_to_write: &str) -> Result<()> {
    let dep = string_to_write.as_bytes();
    file.write_all(dep)?;
    Ok(())
}

fn write_file(file_name: &Path, deps: Vec<PathBuf>) -> Result<()> {
    // Open a file in write-only mode
    let mut file = File::create(&file_name)?;

    let file_name = format!("{}:", file_name.file_name().unwrap().to_str().unwrap());
    write_to_file(&mut file, &file_name)?;

    for dep in deps {
        let dep_string = format!(" {}", dep.into_os_string().into_string().unwrap());
        write_to_file(&mut file, &dep_string)?;
    }

    Ok(())
}

pub fn makedeps(yaml_file: &Path, deps_file: &Path) -> Result<()> {
    let f = File::open(yaml_file)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let mut docs = YamlLoader::load_from_str(&contents)?;
    match &mut docs[0] {
        Yaml::Hash(root) => {
            root.insert(
                Yaml::String("_path".into()),
                Yaml::String(yaml_file.to_str().unwrap().into()),
            );

            let deps = yaml_includes(root)?;

            write_file(deps_file, deps)?;
            Ok(())
        }
        _ => Err(anyhow!("Incorrect yaml {:?}", yaml_file)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_makedeps() -> Result<()> {
        // Create a directory inside of `std::env::temp_dir()`
        let out_dir = tempdir()?;
        let deps_file = out_dir.path().join("test.d");

        let test_dir = test_utils::res_dir().join(Path::new("makedeps"));
        let yaml_file = test_dir.join(Path::new("test.yaml"));

        makedeps(&yaml_file, &deps_file)?;

        let deps: String = fs::read_to_string(deps_file)?.parse()?;
        let exp_string = format!(
            "test.d: {} {}",
            test_dir.join(Path::new("sub-tests/inc1.yaml")).display(),
            test_dir.join(Path::new("sub-tests/inc2.yaml")).display()
        );

        assert_eq!(deps, exp_string);

        Ok(())
    }
}
