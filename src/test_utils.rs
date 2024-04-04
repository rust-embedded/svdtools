use crate::patch::device::DeviceExt;
use anyhow::{anyhow, Context, Result};
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

/// Execute the test found in the specified res/ subdirectory.
///
/// This runs the patch.yaml file in the specified subdirectory, and checks
/// that the results match the expected contents found in the expected.svd file.
pub fn test_expected(test_subdir: &Path) -> Result<()> {
    // Run the patch
    let (mut device, yaml) = get_patcher(test_subdir)?;
    device
        .process(&yaml, &Default::default())
        .context("processing patch.yaml")?;

    // Load the expected contents
    let expected_file = res_dir().join(test_subdir.join("expected.svd"));
    let f = File::open(&expected_file)
        .with_context(|| format!("opening {}", expected_file.display()))?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let expected = svd_parser::parse(&contents)
        .with_context(|| format!("parsing {}", expected_file.display()))?;

    if device != expected {
        // Include a diff of the changes in the error
        let config = svd_encoder::Config::default();
        let dev_text = svd_encoder::encode_with_config(&device, &config)?;
        let expected_text = svd_encoder::encode_with_config(&expected, &config)?;
        let diff = similar::TextDiff::from_lines(&expected_text, &dev_text);
        Err(anyhow!(
            "patch did not produce expected results:\n{}",
            diff.unified_diff()
        ))
    } else {
        Ok(())
    }
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
