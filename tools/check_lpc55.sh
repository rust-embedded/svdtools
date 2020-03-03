#!/bin/bash

set -e

source venv/bin/activate

git clone https://github.com/nickray/lpc55-pacs --depth 1

make -C lpc55-pacs/ patch generate
make -C lpc55-pacs/ generate

rm -rf lpc55-pacs
