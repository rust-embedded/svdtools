# Changelog

## [Unreleased]

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

## [v0.0.1] 2019-08-17

* Initial release, importing from `stm32-rs/scripts/svdpatch.py`
* Add `click` CLI, to call as `svd patch <yaml-file>`
* Add packaging
