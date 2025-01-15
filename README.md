![GitHub top language](https://img.shields.io/github/languages/top/rust-embedded/svdtools)
![Minimum Supported Rust Version](https://img.shields.io/badge/rustc-1.70+-blue.svg)
[![crates.io](https://img.shields.io/crates/v/svdtools.svg)](https://crates.io/crates/svdtools)
[![crates.io](https://img.shields.io/crates/d/svdtools.svg)](https://crates.io/crates/svdtools)
[![Released API docs](https://docs.rs/svdtools/badge.svg)](https://docs.rs/svdtools)
![Crates.io](https://img.shields.io/crates/l/svdtools)
[![dependency status](https://deps.rs/repo/github/rust-embedded/svdtools/status.svg)](https://deps.rs/repo/github/rust-embedded/svdtools)
[![Continuous integration](https://github.com/rust-embedded/svdtools/workflows/Continuous%20integration/badge.svg)](https://github.com/rust-embedded/svdtools)

# svdtools

**svdtools** is a set of tools for modifying vendor-supplied, often buggy SVD
files. It can be imported as a library for use in other applications, or run
directly via the included `svdtools` CLI utility.

A common use case is patching vendor-supplied SVD files, then applying
[svd2rust](https://github.com/rust-embedded/svd2rust) to the resulting patched
SVD.

This project is developed and maintained by the [Tools team][team].

## Getting Started with Python version

Python 3.6 or newer is required to install and use `svdtools`. To install:

```bash
$ pip3 install --upgrade --user svdtools
```

Once installation has completed, the `svd` utility can be called from the command line.

An example is given in `make example`, which calls
`svd patch example/incomplete-stm32l4x2.yaml` and generates a patched SVD file
`example/stm32l4x2.svd.patched`.

See [Device and Peripheral YAML Format](#device-and-peripheral-yaml-format) for
more information on creating patches.

## Getting Started with Rust version

This crate is guaranteed to compile on stable Rust 1.58.0 and up. To install:

```bash
$ cargo install svdtools
```

Once installation has completed, the `svdtools` utility can be called from the command line. Command line interface is same as CLI for Python version.

## Develop

To each their own, but the intended workflow is as follows:

1. Setup a virtual environment via `make setup`; this also installs the `svd` CLI
2. Activate the virtual environment by running `source venv/bin/activate` (or use [direnv](https://direnv.net/))
3. Iterate, running `make check` and `make fix` as necessary


## Device and Peripheral YAML Format

The patch specifications are in [YAML](https://yaml.org/), and have the following
general format:

```yaml
# Path to the SVD file we're targeting. Relative to this file.
# This must be included only in the device YAML file.
_svd: "../svd/STM32F0x0.svd"

# Include other YAML files. Path relative to this file.
_include:
    - "../peripherals/gpio_v2.yaml"

# Alter top-level information and peripherals for this device
_modify:
    version: 1.1
    description: bla bla
    addressUnitBits: 8
    width: 32
    cpu:
        revision: r1p2
        mpuPresent: true
    # Peripherals can either live directly at this level (but other top-level
    # fields will name match first)
    C_ADC:
        name: ADC_Common
    # Or they can be inside a _peripherals block, to avoid name conflicts.
    _peripherals:
        FSMC:
            description: Flexible static memory controller

            # Multiple address blocks are supported via the addressBlocks list
            # use either addressBlock or addressBlocks, but not both
            addressBlocks:
                -   offset: 0x0
                    size: 0x400
                    usage: "ADC base registers"
                -   offset: 0x1000
                    size: 0x400
                    usage: "ADC extra registers"

# Replace fields based on fully-featured regular expressions.
# Note that that since this supports backreferences, runtime can become excessive
# if matching on too many things.
_replace:
    description:
        "pattern": "replace with"
        "another pattern": "and its replacement"
    PER_EX_1:
        name:
            "unnecessary_prefix_": ""
        _registers:
            REG1:
                name:
                    "_per_ex_1": ""
            "*":
                description:
                    "*": ""
            REG2:
                # Modify fields within a register matched by wildcard, careful here for runtime
                "*":
                    "real_example": "example"

# Add whole new peripherals to this device.
# Incredibly this feature is required.
_add:
    ADC_Common:
        description: ADC Common registers
        groupName: ADC
        baseAddress: 0x40012300
        addressBlock:
            offset: 0x0
            size: 0x400
            usage: "All ADC registers"
        # Multiple address blocks are supported via the addressBlocks list
        addressBlocks:
            -   offset: 0x0
                size: 0x400
                usage: "ADC base registers"
            -   offset: 0x1000
                size: 0x400
                usage: "ADC extra registers"
        registers:
            CSR:
                description: ADC Common status register
                addressOffset: 0x0
                access: read-only
                resetValue: 0x00000000
                fields:
                    OVR3:
                        description: Overrun flag of ADC3
                        bitOffset: 21
                        bitWidth: 1
        interrupts:
            ADC1_2:
                description: ADC global interrupt
                value: 18

# A whole new peripheral can also be created as derivedFrom another peripheral.
_add:
    USART3:
        derivedFrom: USART1
        baseAddress: "0x40004800"
        interrupts:
            USART3:
                description: USART3 global interrupt
                value: 39

# A new peripheral can have all its registers copied from another, in case
# it cannot quite be derivedFrom (e.g. some fields need different enumerated
# values) but it's otherwise almost exactly the same.
# The registers are copied but not name or address or interrupts, which are
# preserved if the target already exists.
_copy:
    ADC3:
        from: ADC2

# The new peripheral can also be copied from another svd file for a different
# device. This is useful when a peripheral is missing in a device but the exact
# same peripheral already exist in another device.
# When copying from another file, all fields including interrupts are copied.
_copy:
    TIM1:
        from: ../svd/stm32f302.svd:TIM1

# Replace peripheral registers by a 'deriveFrom'.
# This is used when e.g. UART4 and UART5 are both independently defined,
# but you'd like to make UART5 be defined as derivedFrom UART4 instead.
_derive:
    # The KEY peripheral looses all its elements but 'interrupt', 'name',
    # and 'baseAddress', and it is derivedFrom the VALUE peripheral.
    # Peripherals that were 'deriveFrom="KEY"' are now 'deriveFrom="VALUE"'.
    UART5: UART4

# Reorder the hierarchy of peripherals with 'deriveFrom'.
# This is used when e.g. I2C1 is marked as derivedFrom I2C3,
# but you'd like to swap that so that I2C3 becomes derivedFrom I2C1.
_rebase:
    # The KEY peripheral steals everything but 'interrupt', 'name',
    # and 'baseAddress' elements from the VALUE peripheral.
    # Peripherals that were 'deriveFrom="VALUE"' are now 'deriveFrom="KEY"'.
    # The VALUE peripheral is marked as derivedFrom the updated KEY.
    I2C1: I2C3

# An STM32 peripheral, matches an SVD <peripheral> tag.
# Does not match any tag with derivedFrom attribute set.
"GPIO*":
    # We can include other YAML files inside this peripheral
    _include:
        - "path/to/file.yaml"

    # Alter fields on existing registers inside this peripheral
    _modify:
        # Rename this badly named register. Takes effect before anything else.
        # Don't use wildcard matches if you are changing the name!
        # We could have specified name or description or other tags to update.
        GPIOB_OSPEEDR:
          name: OSPEEDR
        # Equivalently the register could go in a '_registers' block
        _registers:
            GPIOB_OSPEEDR:
                name: OSPEEDR
        # Change the value of an interrupt in this peripheral
        _interrupts:
            EXTI0:
                value: 101


    # Add new registers and interrupts to this peripheral.
    # Entries are registers by default, which can also go inside a '_registers'
    # block, or interrupts go in an '_interrupts' block.
    _add:
        EXAMPLER:
            description: An example register
            addressOffset: 0x04
            access: read-write
            fields:
                EXR1:
                    description: Example field
                    bitOffset: 16
                    bitWidth: 4
        _registers:
            EXAMPLR2:
                description: Another example register
        _interrupts:
            EXAMPLEI:
                description: An example interrupt
                value: 100

    # Anywhere you can '_add' something, you can also '_delete' it.
    # Wildcards are supported. The value here can be a YAML list of registers
    # to delete (supported for backwards compatibility), or a YAML mapping
    # of lists of registers or interrupts.
    _delete:
        GPIO*_EXTRAR:
        _registers:
            - GPIO*_EXAMPLER
        _interrupts:
            - USART1

    # If registers have unnecessary common prefix/postfix,
    # you can clean it in all registers in peripheral by:
    _strip:
        - "PREFIX_*_"
    _strip_end:
        - "_POSTFIX_"

    # You can collect several same registers into one register array
    # that will be represented with svd2rust as array or elements
    # with one type
    # Minimal version:
    _array:
        ARRAY*: {}

    # You can also use the modifiers shown below:
    _array:
        ARRAY*:
            name: NEW_NAME%s
            _modify:
                FIELD:
                  description: NEWDESC
        OTHER_ARRAY*: {}

    # If you have registers that make up a group and can be repeated,
    # you can collect them into cluster like this:
    _cluster:
        CLUSTER%s:
            FIRST_REG: {}
            SECOND_REG: {}

    # clusters can be expanded into individual registers. The name of the resulting register will be the cluster name, concatenated with the register name.

    _expand_cluster:
        - CLUSTER_ONE*
        - CLUSTER_TWO*

    # When expanding clusters containing more than one element (as specified by <dim>), each register will have substutute [%] in the cluster name with its index number. If the cluster has a dimIndex element, a %s in the  cluster name will be replaced by dimIndex element. [%] is not compatible with dimIndex, as according to SVD 1.3.10
    # The SVD 1.3.10 does not specify a delimiter for expansion, so passing the following parameters to the cluster as a hash will allow you to set the delimiter before and after the index is applied (the default delimiters are "_". You can also force the cluster to apply a zero index to a cluster with a single element by passing in the _zeroindex: true parameter

    _expand_cluster:
        CLUSTER_ONE*:
        CLUSTER_TWO*:
          _preindex: "_"
          _postindex: "_"
          _zeroindex: true


    # if you pass the _noprefix: true parameter to a cluster, the cluster name will not be prefixed with the peripheral name. This is only valid for single element clusters.

    _expand_cluster:
        CLUSTER_ONE*:
          _noprefix: true

    # A register on this peripheral, matches an SVD <register> tag
    MODER:
        # As in the peripheral scope, rename or redescribe a field.
        # Don't use wildcard matches if you are changing the name!
        _modify:
            FIELD:
              description: NEWDESC

              # Change the writeConstraint of a field to enumerateValues
              _write_constraint: "enum"

              # Remove any writeConstraint from this field
              _write_constraint: "none"

              # Change the writeConstraint of a field to a range of values
              _write_constraint: [MINIMUM, MAXIMUM]

        # Add new fields to this register
        _add:
            NEWFIELD:
              description: DESCRIPTION
              bitOffset: 12
              bitWidth: 4
              access: read-write

        # Often fields that should be one contiguous integer are specified
        # as a number of individual bits instead. This merges any matching
        # registers into a single field with the combined bitwidth and lowest
        # bit offset, and the shared description and access.
        _merge:
            - "FIELD*"

        # You can also merge fields with different base name like this:
        _merge:
            FIELD: [FIELD1, FIELD_?]
        # Or like this:
        _merge:
            FIELD:
                - FIELD1
                - FIELD_?
        # Or even like this:
        _merge:
            NEW_FIELD: "FIELD*"

        # A field in this register, matches an SVD <field> tag
        FIELD:
            # You can optionally specify name for `enumeratedValues`
            _name: NAME
            # By giving the field a dictionary we construct an enumerateValues
            VARIANT: [VALUE, DESCRIPTION]
            VARIANT: [VALUE, DESCRIPTION]
            # Use `-1` for "default" variant which will be consider
            # for all other values that are not listed explicitly
            # usually datasheet marks them `0b0xxx`, `0b1x`, etc.
            VARIANT: [-1, DESCRIPTION]

        FIELD:
            # If a field already has enumerateValues, drop them and
            # replace them with entirely new ones.
            _replace_enum:
                VARIANT: [VALUE, DESCRIPTION]
                VARIANT: [VALUE, DESCRIPTION]

        # Another field. A list of two numbers gives a range writeConstraint.
        FIELD: [MINIMUM, MAXIMUM]

        # Another field with separate enumerated values for read and write
        FIELD:
            _read:
                VARIANT: [VALUE, DESCRIPTION]
                VARIANT: [VALUE, DESCRIPTION]
            _write:
                VARIANT: [VALUE, DESCRIPTION]
                VARIANT: [VALUE, DESCRIPTION]
        # Sometimes fields are to big so we need to split them into smaller fields
        EXTI:
          IMR:
            # This would split MR into MRi where i = 0 ... bitlength
            _split: [MR]
            # This would split CHxFM into CHiFM where i = 0 ... bitlength
            # and use the current bit for the description in each field
            _split:
              CHxFM:
                name: CH%sFM
                description: Processor 2 transmit channel %s free interrupt mask

            # If fields have unnecessary common prefix/postfix,
            # you can clean it in all registers in peripheral by:
            _strip:
                - "PREFIX_*_"
            _strip_end:
                - "_POSTFIX_"

# You can list glob-like rules separated by commas to cover more periperals or registers at time.
# If rule is optional (peripheral may be missing in some devices) add `?~` in the header.
# Don't abuse it. First test not optional rule.
"?~TIM[18],TIM20":
  CR2:
    # Fields also support collecting in arrays
    _array:
      OIS?:
        description: Output Idle state (OC%s output)
      # Optional rules are supported here too
      "?~OIS?N":
        description: Output Idle state (OC%sN output)
```

### Name Matching

Peripheral, register, and field names can be specified:

- Directly (eg. the full name of the peripheral/register/field)
- Using `?` and `*` for single- and multi- character wildcards
- Using `[ABC]` to give a list of possible matching characters
- Using commas to separate a list of possible matches

You must quote the name if using any special characters in YAML.

The enumerated values `On` and `Off` are treated as a boolean in YAML and Python will throw the error:
`AttributeError: 'bool' object has no attribute 'startswith'`, which does not give
you much information about where the error is. To avoid it, surround the values with
quotes like any other special character.

### Style Guide

- Enumerated values should be named in the past tense (*enabled*, *masked*,
etc.)
- Descriptions should start with capital letters and should not end with a period


## License

svdtools is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.


## Contribute

Pull requests are very welcome!

Please apply `black` and `isort` before committing. This can be accomplished by:
- running `make fix`
- running `black svdtools/` and `isort -y --recursive svdtools/`
- installing an editor/IDE plugin

This avoids bikeshedding over formatting issues :)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crate, the [Tools team][team], promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
[team]: https://github.com/rust-embedded/wg#the-tools-team
