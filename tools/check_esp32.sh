#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/esp-rs/esp32 --depth 1

make -C esp32/ patch

rm -rf esp32
