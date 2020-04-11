use crate::patch::yaml_parser::YamlBody;
use crate::patch::{include, yaml_parser};
use anyhow::Result;
use std::error::Error;
use std::io::Write;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

fn write_to_file(file: &mut File, string_to_write: &str) -> Result<()> {
    let dep = string_to_write.as_bytes();
    file.write_all(dep)?;
    Ok(())
}

fn write_file(file_name: &Path, deps: Vec<PathBuf>) -> Result<()> {
    // Open a file in write-only mode, returns `io::Result<File>`
    let mut file = match File::create(&file_name) {
        Err(why) => panic!(
            "couldn't create {}: {}",
            file_name.display(),
            why.description()
        ),
        Ok(file) => file,
    };

    let file_name = format!("{}:", file_name.file_name().unwrap().to_str().unwrap());
    write_to_file(&mut file, &file_name)?;

    for dep in deps {
        let dep_string = format!(" {}", dep.into_os_string().into_string().unwrap());
        write_to_file(&mut file, &dep_string)?;
    }

    Ok(())
}

pub fn makedeps(yaml_file: &Path, deps_file: &Path) {
    let mut yaml: YamlBody = yaml_parser::from_path(yaml_file);

    let yaml_dir = yaml_file.parent().expect("wrong yaml file path");
    let deps = include::yaml_includes(&mut yaml, yaml_dir);

    if let Err(e) = write_file(deps_file, deps) {
        eprintln!(
            "couldn't create {}: {}",
            deps_file.display(),
            e.description()
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn test_dir() -> PathBuf {
        let res_dir: PathBuf = std::env::current_dir().unwrap().join(Path::new("res"));
        res_dir.join(Path::new("makedeps"))
    }

    #[test]
    fn test_makedeps() -> Result<()> {
        // Create a directory inside of `std::env::temp_dir()`
        let out_dir = tempdir()?;
        let deps_file = out_dir.path().join("test.d");

        let test_dir = test_dir();
        let yaml_file = test_dir.join(Path::new("test.yaml"));

        makedeps(&yaml_file, &deps_file);

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
