#!/bin/bash

set -e

source venv/bin/activate

git clone https://github.com/esp-rs/esp32 --depth 1

make -C esp32/ patch

rm -rf esp32