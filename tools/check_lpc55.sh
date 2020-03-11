#!/bin/bash

set -e

make install-svd2rust-form-rustfmt

git clone https://github.com/nickray/lpc55-pacs --depth 1

make -C lpc55-pacs/ patch generate
make -C lpc55-pacs/ generate

rm -rf lpc55-pacs
