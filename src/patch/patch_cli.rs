use anyhow::Result;
use std::path::Path;

pub fn patch(
    yaml_file: &Path,
    out_path: Option<&Path>,
    format_config: Option<&Path>,
) -> Result<()> {
    super::process_file(yaml_file, out_path, format_config)?;
    Ok(())
}
