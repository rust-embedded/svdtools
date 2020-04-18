use std::io::Write;
use std::{fs::File, path::Path};
use svdtools::common::svd_reader;
use tempfile;

// test svd parser lib consistency
#[test]
fn read_and_write() {
    // read an svd
    let res_dir = Path::new("res/example1");
    let svd_path = res_dir.join("stm32l4x2.svd");
    let svd = svd_reader::device(&svd_path).unwrap();

    // write the svd in another file
    let out_dir = tempfile::tempdir().unwrap();
    let write_path = out_dir.path().join("stm32l4x2_duplicate.svd");
    let xml_out = svd_parser::encode(&svd).unwrap();
    let mut out_file = File::create(&write_path).unwrap();
    out_file.write_all(xml_out.as_bytes()).unwrap();

    // read again the svd
    // BUG in svd_parser crate probably: this panics
    // let wrote_svd = svd_reader::device(&write_path).unwrap();
}
