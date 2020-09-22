[![PyPI][pypi-badge]][pypi-url] [![Travis][travis-badge]][travis-url]

# svdtools

**svdtools** is a Python package for modifying vendor-supplied, often buggy SVD
files. It can be imported as a library for use in other applications, or run
directly via the included `svd` CLI utility.

A common use case is patching vendor-supplied SVD files, then applying
[svd2rust](https://github.com/rust-embedded/svd2rust) to the resulting patched
SVD.

[pypi-badge]: https://img.shields.io/pypi/v/svdtools.svg
[pypi-url]: https://pypi.org/project/svdtools/
[travis-badge]: https://travis-ci.com/stm32-rs/svdtools.svg?branch=master
[travis-url]: https://travis-ci.com/stm32-rs/svdtools

## Getting Started

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
                FIELD: [MINIMUM, MAXIMUM]
                FIELD:
                  description: NEWDESC
        OTHER_ARRAY*: {}

    # If you have registers that make up a group and can be repeated,
    # you can collect them into cluster like this:
    _cluster:
        CLUSTER%s:
            FIRST_REG: {}
            SECOND_REG: {}

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

        # A field in this register, matches an SVD <field> tag
        FIELD:
            # By giving the field a dictionary we construct an enumerateValues
            VARIANT: [VALUE, DESCRIPTION]
            VARIANT: [VALUE, DESCRIPTION]

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

            # If fields have unnecessary common prefix/postfix,
            # you can clean it in all registers in peripheral by:
            _strip:
                - "PREFIX_*_"
            _strip_end:
                - "_POSTFIX_"


```

### Name Matching

Peripheral, register, and field names can be specified:

- Directly (eg. the full name of the peripheral/register/field)
- Using `?` and `*` for single- and multi- character wildcards
- Using `[ABC]` to give a list of possible matching characters
- Using commas to separate a list of possible matches

You must quote the name if using any special characters in YAML.

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
