use anyhow::Result;
use std::path::Path;

pub fn patch(yaml_file: &Path) -> Result<()> {
    svdpatch::process_file(yaml_file)?;
    Ok(())
}
