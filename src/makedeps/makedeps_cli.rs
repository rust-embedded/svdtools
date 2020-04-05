use std::{fs::File, path::Path, io::BufReader};
use serde_yaml::Value;


pub fn makedeps(yaml_file: &Path, deps_file: String) {
    println!("{:?}, {}", yaml_file, deps_file);
    let _yaml_file = File::open(&yaml_file).expect("yaml file doesn't exist");
    let reader = BufReader::new(_yaml_file);

    let mut _device: Value  = serde_yaml::from_reader(reader).expect("yaml not formatted correctly");
    // _device._path = yaml_file.into_os_string().into_string().unwrap();
    println!("{:#?}", _device);
}

