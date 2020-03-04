# Changelog

## [Unreleased]

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

## [v0.0.1] 2019-08-17

* Initial release, importing from `stm32-rs/scripts/svdpatch.py`
* Add `click` CLI, to call as `svd patch <yaml-file>`
* Add packaging
