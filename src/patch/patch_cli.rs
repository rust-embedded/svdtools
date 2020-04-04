use crate::common::svd_reader;
use serde_yaml::Mapping;
use serde_yaml::Value;
use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

pub fn yaml_includes(parent: &Mapping) -> Vec<PathBuf> {
    let mut included: Vec<PathBuf> = vec![];
    let include_node = parent.get(&Value::String("_include".to_string()));
    if let Some(incl) = include_node {
        let incl = incl.as_sequence().expect("_include is not a sequence");
        for relpath in incl {
            let relpath = relpath.as_str().expect("bad _include");
            println!("{}", relpath);
            let parent_path = parent
                .get(&Value::String("_path".to_string()))
                .unwrap()
                .as_str()
                .unwrap();
            let path = Path::new(parent_path).join(Path::new(relpath));
            if included.contains(&path) {
                continue;
            }

            let mut yaml: Value = open_yaml(&path);
            if let Value::Mapping(child) = &mut yaml {
                let path_str: String = path.into_os_string().into_string().unwrap();
                child.insert(Value::String("_path".to_string()), Value::String(path_str));
            }
        }
    }
    included
}

fn open_yaml(yaml_file: &Path) -> Value {
    let file = File::open(yaml_file).expect("yaml file doesn't exist");
    let reader = BufReader::new(file);
    serde_yaml::from_reader(reader).expect("yaml not formatted correctly")
}

pub fn patch(yaml_file: &Path) {
    let mut yaml: Value = open_yaml(yaml_file);

    if let Value::Mapping(m) = &mut yaml {
        let svd = m.get(&Value::String("_svd".to_string()));
        let svd = match svd {
            None => panic!("You must have an svd key in the root YAML file"),
            Some(svd) => svd.as_str().unwrap().to_string(),
        };

        let path: String = fs::canonicalize(yaml_file)
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();

        m.insert(Value::String("_path".to_string()), Value::String(path));

        let yaml_dir = yaml_file.parent().unwrap();
        let svdpath = yaml_dir.join(Path::new(&svd));
        let _svdpath_out = svdpath.join(Path::new(".patched"));
        let mut svd_file = File::open(svdpath).expect("svd file doesn't exist");
        let _peripherals = svd_reader::peripherals(&mut svd_file);

        yaml_includes(&m);
    }

    // match &yaml {
    //     Value::Null => println!("null"),
    //     Value::Bool(b) => println!("bool: {}", b),
    //     Value::Number(n) => println!("number: {}", n),
    //     Value::String(s) => println!("string: {}", s),
    //     Value::Sequence(seq) => println!("sequence: {:#?}", seq),
    //     Value::Mapping(m) => println!("mapping: {:#?}", m),
    // }
    // for m in yaml {
    //     println!("entry:");
    //     if m.contains_key("_svd") {
    //         println!("svd ok");
    //     }
    //     println!("{:#?}", yaml);
    // }
}
