use std::path::Path;
use svdtools::common::svd_reader;
use svdtools::patch;

#[test]
fn example1() {
    let test_dir = Path::new("res/example1");
    let expected_svd_path = test_dir.join("expected.svd");
    dbg!(&expected_svd_path);
    let _expected_svd = svd_reader::device(&expected_svd_path);
    patch::patch_cli::patch(&test_dir.join("patch.yaml"));
    let _actual_svd_path = test_dir.join("stm32l4x2.svd.patched");

    // TODO this does not work yet
    //let actual_svd = svd_reader::device(&actual_svd_path);

    // TODO wait until https://github.com/rust-embedded/svd/issues/111 is solved
    //assert_eq!(expected_svd, actual_svd);
}
