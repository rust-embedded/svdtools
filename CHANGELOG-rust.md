# Changelog

This changelog tracks the Rust `svdtools` project. See
[CHANGELOG-python.md](CHANGELOG-python.md) for the Python `svdtools` project.

## [Unreleased]

* Fix `schema_version` in `convert`

## [v0.2.1] 2022-02-12

* Use `svd-encoder` 0.13.1
* Remove register `access` if empty

## [v0.2.0] 2022-01-15

* Use `svd-parser` 0.13.1
* Add `_clear_fields` in `Device` and `Peripheral` (#90)
* Add new `convert` command to convert between SVD (XML), JSON, and YAML (#92)
* Provide option to opt out of regex replace of 0's in description when
  creating arrays by using a custom `description` attribute (#95)

## [v0.1.0] 2021-12-09

* Initial release with feature-parity with the Python project.

[Unreleased]: https://github.com/stm32-rs/stm32-rs/compare/v0.2.1...HEAD
[v0.2.1]: https://github.com/stm32-rs/svdtools/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/stm32-rs/svdtools/compare/35c3a79...v0.2.0
[v0.1.0]: https://github.com/stm32-rs/svdtools/pull/84
