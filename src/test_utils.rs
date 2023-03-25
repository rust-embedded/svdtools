use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use svd_rs::Device;
use yaml_rust::{yaml::Hash, Yaml, YamlLoader};

pub fn res_dir() -> PathBuf {
    std::env::current_dir().unwrap().join(Path::new("res"))
}

pub fn get_patcher(test_subdir: &Path) -> Result<(Device, Hash)> {
    let test_subdir = res_dir().join(test_subdir);
    let yaml_file = test_subdir.join("patch.yaml");
    // Load the specified YAML root file
    let f = File::open(&yaml_file)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let docs = YamlLoader::load_from_str(&contents)?;
    let yaml = docs[0].as_hash().unwrap(); // select the first document

    // Load the specified SVD file
    let svdpath = abspath(
        &yaml_file,
        Path::new(
            yaml.get(&Yaml::String("_svd".into()))
                .unwrap()
                .as_str()
                .ok_or_else(|| anyhow!("You must have an svd key in the root YAML file"))?,
        ),
    );
    let f = File::open(svdpath)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let device = svd_parser::parse(&contents)?;

    Ok((device, yaml.clone()))
}

/// Gets the absolute path of relpath from the point of view of frompath.
fn abspath(frompath: &Path, relpath: &Path) -> PathBuf {
    normpath::BasePath::new(frompath)
        .unwrap()
        .parent()
        .unwrap()
        .unwrap()
        .join(relpath)
        .canonicalize()
        .unwrap()
        .as_path()
        .into()
}
