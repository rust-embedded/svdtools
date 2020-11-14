#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/stm32-rs/stm32-rs --depth 1

make -j2 -C stm32-rs/ patch

rm -rf stm32-rs
