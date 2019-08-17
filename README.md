# svdtools

Python package to handle vendor-supplied, often buggy SVD files.

One use case is to patch them and apply [svd2rust](https://github.com/rust-embedded/svd2rust).

## Install

Run `pip3 install --upgrade --user svdtools`.

Then call `svd` from command line.


## Use

Documentation to be added, for now see [stm32-rs documentation](https://github.com/stm32-rs/stm32-rs#device-and-peripheral-yaml-format).

An example is given in `make example`, which calls `svd patch example/incomplete-stm32l4x2.yaml`
and generates a patched SVD file `example/stm32l4x2.svd.patched`.

## Develop

To each their own, but the intended workflow is:
- setup virtual environment via `make setup`: this does also install the `svd` CLI
- `source venv/bin/activate` (or use [direnv](https://direnv.net/)...)
- iterate, running `make check` and `make fix`


## License

svdtools is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.


## Contribute

Pull requests are very welcome!

Please apply `black` and `isort` before committing, e.g.,
- run `make fix`, or
- install an editor/IDE plugin

This avoids bikeshedding over formatting issues :)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
