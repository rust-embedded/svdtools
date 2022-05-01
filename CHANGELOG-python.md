# Changelog

This changelog tracks the Python `svdtools` project. See
[CHANGELOG-rust.md](CHANGELOG-rust.md) for the Rust `svdtools` project.

## [Unreleased]

## [v0.1.23] 2022-05-01

* Add support for `modifiedWriteValues` & `readAction` for fields

## [v0.1.22] 2022-04-23

* Add support for PyYAML v6 (#96)
* Sanitise enumeratedValues (#103)

## [v0.1.21] 2022-01-15

* Provide option to opt out of regex replace of 0's in description when
  creating arrays by using a custom `description` attribute (#90, #95)
* Add `_clear_fields` in `Device` and `Peripheral` (#93)

## [v0.1.20] 2021-10-06

* Remove displayName from newly derived registers, fixing #83 (#85)
* Detect and reject duplicate YAML keys (#72)

## [v0.1.19] 2021-10-03

* Fix bug in sorting fields without `bitOffset` attribute (#80)
* Add RP2040 PAC to CI testing (#81)

## [v0.1.18] 2021-10-02

* Fix bugs in non-32-bit register bitmask computation and
  single-element array generation (#78)

## [v0.1.17] 2021-10-02

* Support non-32-bit registers when computing bitmasks (#76)
* Support braceexpand-style expansions in name specifiers (#75)
* Improve support of single-item arrays (#74)

## [v0.1.16] 2021-08-14

* Sort fields using natural sort order when deriving enumeratedValues,
  so the base field is now the first numerically (#66)
* Fix bug in `_split` where bit offsets always started at 0, even if the
  field did not start at bit 0 (#68)
* Allow specifying a custom name and description for split fields (#69)
* Support specifying arrays of fields to merge and providing the new merged
  name (#70)

## [v0.1.15] 2021-07-22

* Add support for field arrays to `svd mmap` command

## [v0.1.14] 2021-05-28

* Add `_clear` for deleting all `enumeratedValues` from field
* Support for collecting fields in field arrays
* Deriving fields

## [v0.1.13] 2021-04-16

* Fix use of `vendorExtensions` tag in SVD files (#53)
* Preserve top-level comments in SVD files by swapping to LXML (#52)
* Add registers element if missing (#22)

## [v0.1.12] 2021-01-31

* Support `bitRange` and `msb`/`lsb` as well as `bitOffset` and `bitWidth`
  in field elements (#46).

## [v0.1.11] 2021-01-08

* Add `cpu` top-level element if it does not already exist when modifying it.

## [v0.1.10] 2020-11-14

* Fix identifying dimIndex when matching with multiple comma-separated
  strings (#42)

## [v0.1.9] 2020-09-22

* Fix a bug in `_copy` which resulted in the wrong interrupts ending up
  in the newly copied peripheral.

## [v0.1.8] 2020-09-20

* Permit adding/modifying/deleting interrupts from derived peripherals
* Sort output SVDs into correct order for SVD schema
* Fix bug where addressBlock modifications could lead to duplicate elements
* Allow register `_modify` to create new tags, as done on field in 0.1.7

## [v0.1.7] 2020-09-15

* Allow overwriting enumeratedValues with `_replace_enum`
* Allow field `_modify` to create new tags
* Add `_write_constraint` field modifier
* Allow register `_modify` to create new tags
* Check for existing enums in fields with derived enumeratedValues

## [v0.1.6] 2020-06-16

* Add the ability to modify clusters
* Allow patterns in `_strip`/`_strip_end`

## [v0.1.5] 2020-03-20

* Manipulate multiple peripheral address blocks - @arjanmels

## [v0.1.4] 2020-02-18

* Revert v0.1.3 changes as they broke stm32-rs builds.

## [v0.1.3] 2020-02-18

* Iterate through derived peripherals when processing a device.

## [v0.1.2] 2020-01-29

* Fixed behavior of \_strip\_end  - @ahepp

## [v0.1.1] 2020-01-26

* Backport changes for deriving registers from stm32-rs svdpatch.py - @jessebraham

## [v0.1.0] 2020-01-20

* Backport two changes to stm32-rs svdpatch.py
* Set minor version so stm32-rs can potentially rely on this

## [v0.0.4] 2020-01-12

* Add `strip` & `_strip_end` patching options for stripping bitfields

## [v0.0.3] 2020-01-10

* Add missing `black` and `isort` requirements - @jessebraham
* Add `_strip_end` as an option for patching - @jessebraham

## [v0.0.2] 2019-08-20

* Import the current `stm32-rs/scripts/svdpatch.py` instead of an old one

## v0.0.1 2019-08-17

* Initial release, importing from `stm32-rs/scripts/svdpatch.py`
* Add `click` CLI, to call as `svd patch <yaml-file>`
* Add packaging

[Unreleased]: https://github.com/stm32-rs/svdtools/compare/v0.1.23...HEAD
[v0.1.23]: https://github.com/stm32-rs/svdtools/compare/v0.1.22...v0.1.23
[v0.1.22]: https://github.com/stm32-rs/svdtools/compare/v0.1.21...v0.1.22
[v0.1.21]: https://github.com/stm32-rs/svdtools/compare/v0.1.20...v0.1.21
[v0.1.20]: https://github.com/stm32-rs/svdtools/compare/v0.1.19...v0.1.20
[v0.1.19]: https://github.com/stm32-rs/svdtools/compare/v0.1.18...v0.1.19
[v0.1.18]: https://github.com/stm32-rs/svdtools/compare/v0.1.17...v0.1.18
[v0.1.17]: https://github.com/stm32-rs/svdtools/compare/v0.1.16...v0.1.17
[v0.1.16]: https://github.com/stm32-rs/svdtools/compare/v0.1.15...v0.1.16
[v0.1.15]: https://github.com/stm32-rs/svdtools/compare/v0.1.14...v0.1.15
[v0.1.14]: https://github.com/stm32-rs/svdtools/compare/v0.1.13...v0.1.14
[v0.1.13]: https://github.com/stm32-rs/svdtools/compare/v0.1.12...v0.1.13
[v0.1.12]: https://github.com/stm32-rs/svdtools/compare/v0.1.11...v0.1.12
[v0.1.11]: https://github.com/stm32-rs/svdtools/compare/v0.1.10...v0.1.11
[v0.1.10]: https://github.com/stm32-rs/svdtools/compare/v0.1.9...v0.1.10
[v0.1.9]: https://github.com/stm32-rs/svdtools/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/stm32-rs/svdtools/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/stm32-rs/svdtools/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/stm32-rs/svdtools/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/stm32-rs/svdtools/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/stm32-rs/svdtools/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/stm32-rs/svdtools/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/stm32-rs/svdtools/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/stm32-rs/svdtools/compare/v0.1.0...v0.1.1
[v0.1.0]: https://github.com/stm32-rs/svdtools/compare/v0.0.4...v0.1.0
[v0.0.4]: https://github.com/stm32-rs/svdtools/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/stm32-rs/svdtools/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/stm32-rs/svdtools/compare/v0.0.1...v0.0.2
