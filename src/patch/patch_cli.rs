use super::Config;
use anyhow::Result;
use std::path::Path;

pub fn patch(
    yaml_file: &Path,
    out_path: Option<&Path>,
    format_config: Option<&Path>,
    config: &Config,
) -> Result<()> {
    super::process_file(yaml_file, out_path, format_config, config)?;
    Ok(())
}

pub fn expand_patch(yaml_file: &Path) -> Result<String> {
    let doc = super::load_patch(yaml_file)?;
    let mut out_str = String::new();
    let mut emitter = yaml_rust::YamlEmitter::new(&mut out_str);
    emitter.dump(&doc).unwrap();
    Ok(out_str)
}
