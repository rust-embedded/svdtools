use std::path::{Path, PathBuf};

pub fn res_dir() -> PathBuf {
    std::env::current_dir().unwrap().join(Path::new("res"))
}
