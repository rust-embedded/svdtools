import os.path

import yaml

from ..makedeps import main as makedeps


def test_makedeps(tmpdir):
    yaml_file = os.path.join(tmpdir, "test.yaml")
    inc1_file = os.path.join(tmpdir, "inc1.yaml")
    inc2_file = os.path.join(tmpdir, "inc2.yaml")
    deps_file = os.path.join(tmpdir, "test.d")

    device = {"_include": ["inc1.yaml"]}
    inc1 = {"_include": ["inc2.yaml"]}
    inc2 = {}

    with open(yaml_file, "w") as f:
        yaml.safe_dump(device, f)
    with open(inc1_file, "w") as f:
        yaml.safe_dump(inc1, f)
    with open(inc2_file, "w") as f:
        yaml.safe_dump(inc2, f)

    makedeps(yaml_file, deps_file)

    with open(deps_file) as f:
        deps = f.read()

    assert deps == f"{deps_file}: {inc1_file} {inc2_file}\n"
