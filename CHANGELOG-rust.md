# Changelog

This changelog tracks the Rust `svdtools` project. See
[CHANGELOG-python.md](CHANGELOG-python.md) for the Python `svdtools` project.

## [Unreleased]

## [v0.5.0] 2025-05-25

* Add `bin` feature (enabled by default), make some dependencies optional
* Skip adding `enumeratedValues` & `writeConstraint` to derived fields

## [v0.4.6] 2025-04-03

* Fixed compilation with new `rust`

## [v0.4.5] 2025-03-11

* Calculate `_add` position for peripherals and registers

## [v0.4.4] 2025-02-08

* Add `prefix` and `suffix` as opposite to `strip` and `strip_end`

## [v0.4.3] 2025-01-31

* Allow shorthand when `_derive` across clusters

## [v0.4.2] 2025-01-25

* Fix optional `fspec` in `set_field_modified_write_values`

## [v0.4.1] 2025-01-21

* `html`: field `readAction` and `modifiedWriteValues` in `access`
* `patch`: allow processing without path dependencies

## [v0.4.0] 2025-01-06

* **breaking** Support "?~" in field `_modify` & `_derive`
* Keep position when merge fields

## [v0.3.21] 2024-12-31

* `_derive` field

## [v0.3.20] 2024-11-14

* Implement `_expand_cluster`
* Support derived registers/fields in `svdtools html`

## [v0.3.19] 2024-10-18

* Fix deletion childrens on cluster modify
* Sugar for simple `_split` and `_merge`
* Show number of covered fields in `mmaps`
* Sugar for simple `_split` and `_merge` 

## [v0.3.18] 2024-08-10

* Replace `yaml-rust` with `yaml-rust2`. Check for duplicate keys and other YAML parse errors
* Prevent collapsing columns in HTML `register-map-table` with `min-width`
* Protect from using one `enumeratedValues` in fields with different width

## [v0.3.17] 2024-07-05

* Support "isDefault" enum value in `svdtools html`

## [v0.3.16] 2024-07-03

* Add possibility to add field arrays

## [v0.3.15] 2024-07-02

* Add `info` tool, only with `device-name` for now
* Add `expand-patch` tool to show full patch rule with all includes

## [v0.3.14] 2024-04-04

* If there is no path to interpolate, show unmodified `description`.
* Support write constraints when defining new register fields
* Allow absolute identifiers in `_derive` register

## [v0.3.13] 2024-03-29

* Interpolate path and name in `description` and `derivedFrom`
* Allow specs started with `_` (missing part)
* Allow 1 string `_include`

## [v0.3.12] 2024-03-23

* `dimArrayIndex` support
* Allow to modify registers in `_cluster:`
* Add addressOffset check in `collect_in_cluster`
* Refactor `RegisterBlockExt`, use `BlockPath` for better errors
* Allow specs started with `_`
* Allow process, `_delete` and `_modify` cluster without `_clusters` specifier

## [v0.3.11] 2024-03-06

* Add `_expand_array`
* Allow register arrays in `collect_in_cluster`
* Custom `dimIncrement` for 1-element `cluster` or `array`

## [v0.3.10] 2024-02-26

* Allow to specify `name` for `enumeratedValues`
* Avoid unneded whitespaces in generated `html`

## [v0.3.9] 2024-01-19

* Use `<details>` instead of JavaScript in `html` template
* Fix escape special characters on Windows

## [v0.3.8] 2023-12-23

* Fix #176 in `collect_in_cluster`

## [v0.3.7] 2023-12-22

* Support `bitRange` and `msb+lsb` in field `_modify`
* Support `_include` in peripherals in `device.yaml`
* Add `--enum_derive` flag
* Strip `alternateRegister` too
* Add `modifiedWriteValues` and `readAction` field patch (#156)
* Ignore rule if starts with "?~" and no matched instances
* Fix #144
* Flag to check for errors after patching

## [v0.3.6] 2023-11-01

* Fix #182

## [v0.3.5] 2023-11-30

* Move field with derived enums before other
* `-1` for default enum value
* Update `displayName` during collect, improve searching common `description`
* mmaps: peripheral arrays, bump `svd` crates
* patch: `--show-patch-on-error` prints yaml patches on error

## [v0.3.4] 2023-10-14

* Revert #145
* Improve "Could not find errors"
* use register size as dimIncrement for 1-element arrays
* Replace spec indices search with regex
* modify writeConstraint for register

## [v0.3.3] 2023-10-02

* Fast fix for #161

## [v0.3.2] 2023-10-01

* `_modify` `derivedFrom` for peripherals, clusters, registers and fields
* fix field bit range in `svdtools html`

## [v0.3.1] 2023-09-19

* add `svdtools html` and `svdtools htmlcompare` tools from `stm32-rs`
* update `svd-encoder`, `--format-config` and optional `out_path` for `patch`
* add field name in enumeratedValues derive path

## [v0.3.0] 2023-03-27

* cluster add/modify
* use `normpath` instead of std::fs::canonicalize

## [v0.2.8] 2023-01-28

* patch: added possibility to modify dim elements (arrays)
* mmap: replace %s in field array

## [v0.2.7] 2022-09-18

* Print svdtools version on error, update dependencies
* Check `_delete`, `_strip`, etc. on array of strings

## [v0.2.6] 2022-08-21

**Breaking changes**:

* Move `_strip`, `_strip_end` before `_modify` (#89)
    * Existing patch files may need updating to refer to the stripped
      versions of names being modified
* Allow `_derive` to rename derived peripherals, optionally specify a new base
    address and description (#118)
    * If registers were being copied and modified, use `_copy` instead of
      `_derive` for those peripherals.

Other changes:

* Improve error messages on missing files (#117)
* Fix help documentation for `svdtools patch` command (#119)

## [v0.2.5] 2022-07-23

* update `svd-rs` crates to 0.14
* `convert`: Add `format_config` option

## [v0.2.4] 2022-05-15

* Added action to build binaries and release for every version tag and latest commit
* Use `svd-parser` 0.13.4, add `expand_properties` option in `convert`
* `patch`: check enum `usage`, don't add it if unneeded

## [v0.2.3] 2022-05-01

* Add support for `modifiedWriteValues` & `readAction` for fields

## [v0.2.2] 2022-04-23

* Use `svd-encoder` 0.13.2
* Support `expand` when processing SVD files (#104)
* Sanitize enumeratedValues (#103)

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

[Unreleased]: https://github.com/rust-embedded/svdtools/compare/v0.5.0...HEAD
[v0.5.0]: https://github.com/rust-embedded/svdtools/compare/v0.4.6...v0.5.0
[v0.4.6]: https://github.com/rust-embedded/svdtools/compare/v0.4.5...v0.4.6
[v0.4.5]: https://github.com/rust-embedded/svdtools/compare/v0.4.4...v0.4.5
[v0.4.4]: https://github.com/rust-embedded/svdtools/compare/v0.4.3...v0.4.4
[v0.4.3]: https://github.com/rust-embedded/svdtools/compare/v0.4.2...v0.4.3
[v0.4.2]: https://github.com/rust-embedded/svdtools/compare/v0.4.1...v0.4.2
[v0.4.1]: https://github.com/rust-embedded/svdtools/compare/v0.4.0...v0.4.1
[v0.4.0]: https://github.com/rust-embedded/svdtools/compare/v0.3.21...v0.4.0
[v0.3.21]: https://github.com/rust-embedded/svdtools/compare/v0.3.20...v0.3.21
[v0.3.20]: https://github.com/rust-embedded/svdtools/compare/v0.3.19...v0.3.20
[v0.3.19]: https://github.com/rust-embedded/svdtools/compare/v0.3.18...v0.3.19
[v0.3.18]: https://github.com/rust-embedded/svdtools/compare/v0.3.17...v0.3.18
[v0.3.17]: https://github.com/rust-embedded/svdtools/compare/v0.3.16...v0.3.17
[v0.3.16]: https://github.com/rust-embedded/svdtools/compare/v0.3.15...v0.3.16
[v0.3.15]: https://github.com/rust-embedded/svdtools/compare/v0.3.14...v0.3.15
[v0.3.14]: https://github.com/rust-embedded/svdtools/compare/v0.3.13...v0.3.14
[v0.3.13]: https://github.com/rust-embedded/svdtools/compare/v0.3.12...v0.3.13
[v0.3.12]: https://github.com/rust-embedded/svdtools/compare/v0.3.11...v0.3.12
[v0.3.11]: https://github.com/rust-embedded/svdtools/compare/v0.3.10...v0.3.11
[v0.3.10]: https://github.com/rust-embedded/svdtools/compare/v0.3.9...v0.3.10
[v0.3.9]: https://github.com/rust-embedded/svdtools/compare/v0.3.8...v0.3.9
[v0.3.8]: https://github.com/rust-embedded/svdtools/compare/v0.3.7...v0.3.8
[v0.3.7]: https://github.com/rust-embedded/svdtools/compare/v0.3.6...v0.3.7
[v0.3.6]: https://github.com/rust-embedded/svdtools/compare/v0.3.5...v0.3.6
[v0.3.5]: https://github.com/rust-embedded/svdtools/compare/v0.3.4...v0.3.5
[v0.3.4]: https://github.com/rust-embedded/svdtools/compare/v0.3.3...v0.3.4
[v0.3.3]: https://github.com/rust-embedded/svdtools/compare/v0.3.2...v0.3.3
[v0.3.2]: https://github.com/rust-embedded/svdtools/compare/v0.3.1...v0.3.2
[v0.3.1]: https://github.com/rust-embedded/svdtools/compare/v0.3.0...v0.3.1
[v0.3.0]: https://github.com/rust-embedded/svdtools/compare/v0.2.8...v0.3.0
[v0.2.8]: https://github.com/rust-embedded/svdtools/compare/v0.2.7...v0.2.8
[v0.2.7]: https://github.com/rust-embedded/svdtools/compare/v0.2.6...v0.2.7
[v0.2.6]: https://github.com/rust-embedded/svdtools/compare/v0.2.5...v0.2.6
[v0.2.5]: https://github.com/rust-embedded/svdtools/compare/v0.2.4...v0.2.5
[v0.2.4]: https://github.com/rust-embedded/svdtools/compare/v0.2.3...v0.2.4
[v0.2.3]: https://github.com/rust-embedded/svdtools/compare/v0.2.2...v0.2.3
[v0.2.2]: https://github.com/rust-embedded/svdtools/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/rust-embedded/svdtools/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/rust-embedded/svdtools/compare/35c3a79...v0.2.0
[v0.1.0]: https://github.com/rust-embedded/svdtools/pull/84
