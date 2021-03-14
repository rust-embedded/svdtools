"""
interrupts.py
Copyright 2018-2020 Adam Greig
Licensed under the MIT and Apache 2.0 licenses. See LICENSE files for details.
"""

import lxml.etree as ET


def parse_device(svd_file):
    interrupts = {}
    tree = ET.parse(svd_file)
    dname = tree.find("name").text
    for ptag in tree.iter("peripheral"):
        pname = ptag.find("name").text
        for itag in ptag.iter("interrupt"):
            name = itag.find("name").text
            value = itag.find("value").text
            maybe_desc = itag.find("description")
            desc = maybe_desc.text.replace("\n", " ") if maybe_desc is not None else ""
            interrupts[int(value)] = {"name": name, "desc": desc, "pname": pname}
    return dname, interrupts


def main(svd_file, gaps=True):
    name, interrupts = parse_device(svd_file)
    missing = set()
    lastint = -1
    results = []
    for val in sorted(interrupts.keys()):
        for v in range(lastint + 1, val):
            missing.add(v)
        lastint = val
        i = interrupts[val]
        results.append(f"{val} {i['name']}: {i['desc']} (in {i['pname']})")
    if gaps:
        results.append("Gaps: " + ", ".join(str(x) for x in sorted(missing)))
    return "\n".join(results)
