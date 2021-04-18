# Changelog

## [Unreleased]

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

[Unreleased]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.13...HEAD
[v0.1.13]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.12...v0.1.13
[v0.1.12]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.11...v0.1.12
[v0.1.11]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.10...v0.1.11
[v0.1.10]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.9...v0.1.10
[v0.1.9]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/stm32-rs/stm32-rs/compare/v0.1.0...v0.1.1
[v0.1.0]: https://github.com/stm32-rs/stm32-rs/compare/v0.0.4...v0.1.0
[v0.0.4]: https://github.com/stm32-rs/stm32-rs/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/stm32-rs/stm32-rs/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/stm32-rs/stm32-rs/compare/v0.0.1...v0.0.2
