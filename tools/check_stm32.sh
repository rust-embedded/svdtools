#!/bin/bash

set -e

source venv/bin/activate

git clone https://github.com/stm32-rs/stm32-rs --depth 1

make -C stm32-rs/ patch

rm -rf stm32-rs