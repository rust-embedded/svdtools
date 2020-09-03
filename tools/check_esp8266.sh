#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/esp-rs/esp8266 --depth 1

make -C esp8266/ patch

rm -rf esp8266
