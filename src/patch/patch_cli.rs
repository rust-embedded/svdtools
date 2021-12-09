use anyhow::Result;
use std::path::Path;

pub fn patch(yaml_file: &Path) -> Result<()> {
    super::process_file(yaml_file)?;
    Ok(())
}
