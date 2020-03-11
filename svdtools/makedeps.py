"""
makedeps.py
Copyright 2017, 2020 Adam Greig
Licensed under the MIT and Apache 2.0 licenses. See LICENSE files for details.
"""

import yaml

from . import patch


def main(yaml_file, deps_file):
    with open(yaml_file, encoding="utf-8") as f:
        device = yaml.safe_load(f)
    device["_path"] = yaml_file
    deps = patch.yaml_includes(device)
    with open(deps_file, "w") as f:
        f.write("{}: {}\n".format(deps_file, " ".join(deps)))
