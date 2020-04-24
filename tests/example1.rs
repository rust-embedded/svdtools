use std::path::Path;
use svdtools::{common::svd_reader, patch};

#[test]
fn example1() {
    let test_dir = Path::new("res/example1");
    let expected_svd_path = test_dir.join("expected.svd");
    let _expected_svd = svd_reader::device(&expected_svd_path).unwrap();

    patch::patch_cli::patch(&test_dir.join("patch.yaml")).unwrap();
    let actual_svd_path = test_dir.join("stm32l4x2.svd.patched");

    let _actual_svd = svd_reader::device(&actual_svd_path).unwrap();

    // TODO wait until https://github.com/rust-embedded/svd/issues/111 is solved
    //assert_eq!(expected_svd, actual_svd);
}
