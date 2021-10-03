#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/rp-rs/rp2040-pac --depth 1

pushd rp2040-pac
svd patch svd/rp2040.yaml
svd2rust -i svd/rp2040.svd.patched
form -i lib.rs -o src
rm lib.rs
cargo fmt
cargo check
popd

rm -rf rp2040-pac
