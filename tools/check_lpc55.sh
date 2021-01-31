#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/lpc55/lpc55-pac --depth 1

make -C lpc55-pac/ patch generate
make -C lpc55-pac/ generate

rm -rf lpc55-pacs
