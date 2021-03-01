#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/lpc55/lpc55-pac --depth 1

make -C lpc55-pac/ check

rm -rf lpc55-pac
