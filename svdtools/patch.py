"""
svdpatch.py

Copyright 2017-2019 Adam Greig.
Licensed under the MIT and Apache 2.0 licenses. See LICENSE files for details.
"""

import copy
import fnmatch
import os.path
import re
from collections import OrderedDict
from fnmatch import fnmatchcase

import lxml.etree as ET
import yaml
from braceexpand import braceexpand

DEVICE_CHILDREN = [
    "vendor",
    "vendorID",
    "name",
    "series",
    "version",
    "description",
    "licenseText",
    "headerSystemFilename",
    "headerDefinitionsPrefix",
    "addressUnitBits",
    "width",
    "size",
    "access",
    "protection",
    "resetValue",
    "resetMask",
]


# Set up pyyaml to use ordered dicts so we generate the same
# XML output each time, and detect and refuse duplicate keys.
def dict_constructor(loader, node, deep=False):
    mapping = set()
    for key_node, _ in node.value:
        key = loader.construct_object(key_node, deep=deep)
        start = node.start_mark
        assert key not in mapping, f"duplicate key '{key}' found {start}"
        mapping.add(key)
    return OrderedDict(loader.construct_pairs(node))


_mapping_tag = yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG
yaml.add_constructor(_mapping_tag, dict_constructor, yaml.SafeLoader)


def matchname(name, spec):
    """Check if name matches against a specification."""
    if spec.startswith("_"):
        return False
    if "{" in spec:
        return any(fnmatchcase(name, subspec) for subspec in braceexpand(spec))
    else:
        return any(fnmatchcase(name, subspec) for subspec in spec.split(","))


def matchsubspec(name, spec):
    """If a name matches a specification, return the first sub-specification that it
    matches.
    """
    if not matchname(name, spec):
        return None
    if "{" in spec:
        for subspec in braceexpand(spec):
            if fnmatchcase(name, subspec):
                return subspec
    else:
        for subspec in spec.split(","):
            if fnmatchcase(name, subspec):
                return subspec
    return None


def create_regex_from_pattern(substr, strip_end):
    """Create regex from pattern to match start or end of string."""
    regex = fnmatch.translate(substr)
    # make matching non-greedy
    regex = re.sub("\\*", "*?", regex)
    # change to start of string search
    if not strip_end:
        regex = "^" + re.sub("\\\\Z$", "", regex)
    return re.compile(regex)


def abspath(frompath, relpath):
    """Gets the absolute path of relpath from the point of view of frompath."""
    basepath = os.path.realpath(os.path.join(os.path.abspath(frompath), os.pardir))
    return os.path.normpath(os.path.join(basepath, relpath))


def update_dict(parent, child):
    """
    Recursively merge child.key into parent.key, with parent overriding.
    """
    for key in child:
        if key == "_path" or key == "_include":
            continue
        elif key in parent:
            if isinstance(parent[key], list):
                parent[key] += child[key]
            elif isinstance(parent[key], dict):
                update_dict(parent[key], child[key])
        else:
            parent[key] = child[key]


def yaml_includes(parent):
    """Recursively loads any included YAML files."""
    included = []
    for relpath in parent.get("_include", []):
        path = abspath(parent["_path"], relpath)
        if path in included:
            continue
        with open(path, encoding="utf-8") as f:
            child = yaml.safe_load(f)
        child["_path"] = path
        included.append(path)
        # Process any peripheral-level includes in child
        for pspec in child:
            if not pspec.startswith("_") and "_include" in child[pspec]:
                child[pspec]["_path"] = path
                included += yaml_includes(child[pspec])
        # Process any top-level includes in child
        included += yaml_includes(child)
        update_dict(parent, child)
    return included


def make_write_constraint(wc_range):
    """Given a (min, max), returns a writeConstraint Element."""
    wc = ET.Element("writeConstraint")
    r = ET.SubElement(wc, "range")
    minimum = ET.SubElement(r, "minimum")
    minimum.text = str(wc_range[0])
    maximum = ET.SubElement(r, "maximum")
    maximum.text = str(wc_range[1])
    wc.tail = "\n            "
    return wc


def make_enumerated_values(name, values, usage="read-write"):
    """
    Given a name and a dict of values which maps variant names to (value,
    description), returns an enumeratedValues Element.
    """
    ev = ET.Element("enumeratedValues")
    usagekey = {"read": "R", "write": "W"}.get(usage, "")
    ET.SubElement(ev, "name").text = name + usagekey
    ET.SubElement(ev, "usage").text = usage
    if len(set(v[0] for v in values.values())) != len(values):
        raise ValueError("enumeratedValue {}: can't have duplicate values".format(name))
    if name[0] in "0123456789":
        raise ValueError("enumeratedValue {}: can't start with a number".format(name))
    for vname in values:
        if vname.startswith("_"):
            continue
        if vname[0] in "0123456789":
            raise ValueError(
                "enumeratedValue {}.{}: can't start with a number".format(name, vname)
            )
        value, description = values[vname]
        if not description:
            raise ValueError(
                "enumeratedValue {}: can't have empty description"
                " for value {}".format(name, value)
            )
        el = ET.SubElement(ev, "enumeratedValue")
        ET.SubElement(el, "name").text = vname
        ET.SubElement(el, "description").text = description
        ET.SubElement(el, "value").text = str(value)
    ev.tail = "\n            "
    return ev


def make_derived_enumerated_values(name):
    """Returns an enumeratedValues Element which is derivedFrom name."""
    evd = ET.Element("enumeratedValues", {"derivedFrom": name})
    evd.tail = "\n            "
    return evd


def spec_ind(spec):
    """
    Find left and right indices of enumeration token in specification string.
    """
    li1 = spec.find("*")
    li2 = spec.find("?")
    li3 = spec.find("[")
    li = li1 if li1 > -1 else li2 if li2 > -1 else li3 if li3 > -1 else None
    ri1 = spec[::-1].find("*")
    ri2 = spec[::-1].find("?")
    ri3 = spec[::-1].find("]")
    ri = ri1 if ri1 > -1 else ri2 if ri2 > -1 else ri3 if ri3 > -1 else None
    return li, ri


def check_offsets(offsets, dimIncrement):
    for o1, o2 in zip(offsets[:-1], offsets[1:]):
        if o2 - o1 != dimIncrement:
            return False
    return True


def check_bitmasks(masks, mask):
    for m in masks:
        if m != mask:
            return False
    return True


def get_field_offset_width(ftag):
    """
    Return the offset and width of a field, parsing either bitOffset+bitWidth,
    or a bitRange tag, or lsb and msb tags.
    """
    if ftag.findtext("bitOffset") is not None:
        offset = int(ftag.findtext("bitOffset"), 0)
        width = int(ftag.findtext("bitWidth"), 0)
    elif ftag.findtext("bitRange") is not None:
        msb, lsb = ftag.findtext("bitRange")[1:-1].split(":")
        offset = int(lsb, 0)
        width = int(msb, 0) - offset + 1
    elif ftag.findtext("lsb") is not None:
        lsb = int(ftag.findtext("lsb"), 0)
        msb = int(ftag.findtext("msb"), 0)
        offset = lsb
        fwidth = msb - lsb + 1
    return offset, width


def sort_element(tag):
    """
    The SVD schema requires that all child elements appear in a defined order
    inside their parent element.
    However, new elements may have been been specified in any order,
    so we sort all elements after processing a file.
    """
    arr = ("dim", "dimIncrement", "dimIndex", "dimName", "dimArrayIndex")
    acc = ("size", "access", "protection", "resetValue", "resetMask")
    orders = {
        "enumeratedValue": ("name", "description", "value", "isDefault"),
        "enumeratedValues": ("name", "headerEnumName", "usage", "enumeratedValue"),
        "field": arr
        + (
            "name",
            "description",
            "bitOffset",
            "bitWidth",
            "lsb",
            "msb",
            "bitRange",
            "access",
            "modifiedWriteValues",
            "writeConstraint",
            "readAction",
            "enumeratedValues",
        ),
        "fields": ("field"),
        "writeConstraint": ("writeAsRead", "useEnumeratedValues", "range"),
        "range": ("minimum", "maximum"),
        "register": arr
        + (
            "name",
            "displayName",
            "description",
            "alternateGroup",
            "alternateRegister",
            "addressOffset",
        )
        + acc
        + (
            "dataType",
            "modifiedWriteValues",
            "writeConstraint",
            "readAction",
            "fields",
        ),
        "cluster": arr
        + (
            "name",
            "description",
            "alternateCluster",
            "headerStructName",
            "addressOffset",
        )
        + acc
        + ("register", "cluster"),
        "registers": ("cluster", "register"),
        "interrupt": ("name", "description", "value"),
        "addressBlock": ("offset", "size", "usage", "protection"),
        "peripheral": arr
        + (
            "name",
            "version",
            "description",
            "alternatePeripheral",
            "groupName",
            "prependToName",
            "appendToName",
            "headerStructName",
            "disableCondition",
            "baseAddress",
        )
        + acc
        + ("addressBlock", "interrupt", "registers"),
        "peripherals": ("peripheral"),
        "cpu": (
            "name",
            "revision",
            "endian",
            "mpuPresent",
            "fpuPresent",
            "fpuDP",
            "dspPresent",
            "icachePresent",
            "dcachePresent",
            "itcmPresent",
            "dtcmPresent",
            "vtorPresent",
            "nvicPrioBits",
            "vendorSystickConfig",
            "deviceNumInterrupts",
            "sauNumRegions",
            "sauRegionsConfig",
        ),
        "sauRegionsConfig": ("region"),
        "region": ("base", "limit", "access"),
        "device": (
            "vendor",
            "vendorID",
            "name",
            "series",
            "version",
            "description",
            "licenseText",
            "cpu",
            "headerSystemFilename",
            "headerDefinitionsPrefix",
            "addressUnitBits",
            "width",
        )
        + acc
        + ("peripherals", "vendorExtensions"),
    }
    if tag.tag == "vendorExtensions":
        # We can't sort inside vendorExtensions.
        return
    if len(tag) > 0 and tag.tag not in orders:
        raise UnknownTagError(tag.tag)
    comments = []
    for child in tag:
        if child.tag is ET.Comment:
            comments.append(child)
        elif child.tag not in orders[tag.tag]:
            raise UnknownTagError((tag.tag, child.tag))
    for comment in comments:
        # Remove interior comments, which we cannot sort.
        tag.remove(comment)
    tag[:] = sorted(tag, key=lambda e: orders[tag.tag].index(e.tag))


def sort_recursive(tag):
    sort_element(tag)
    # Don't process children inside vendorExtensions.
    if tag.tag != "vendorExtensions":
        for child in tag:
            sort_recursive(child)


class SvdPatchError(ValueError):
    pass


class RegisterMergeError(SvdPatchError):
    pass


class MissingFieldError(SvdPatchError):
    pass


class MissingRegisterError(SvdPatchError):
    pass


class MissingPeripheralError(SvdPatchError):
    pass


class UnknownTagError(SvdPatchError):
    pass


class Device:
    """Class collecting methods for processing device contents"""

    def __init__(self, device):
        self.device = device

    def iter_peripherals(self, pspec, check_derived=True):
        """Iterates over all peripherals that match pspec."""
        for ptag in self.device.iter("peripheral"):
            name = ptag.find("name").text
            if matchname(name, pspec):
                if check_derived and "derivedFrom" in ptag.attrib:
                    continue
                yield ptag

    def modify_child(self, key, val):
        """Modify key inside device and set it to val."""
        for child in self.device.findall(key):
            child.text = str(val)

    def modify_cpu(self, mod):
        """Modify the `cpu` node inside `device` according to `mod`."""
        cpu = self.device.find("cpu")
        if cpu is None:
            cpu = ET.SubElement(self.device.getroot(), "cpu")
            cpu.tail = "\n  "
        for key, val in mod.items():
            field = cpu.find(key)
            if field is not None:
                field.text = str(val)
            else:
                field = ET.SubElement(cpu, key)
                field.text = str(val)

    def modify_peripheral(self, pspec, pmod):
        """Modify pspec inside device according to pmod."""
        for ptag in self.iter_peripherals(pspec):
            for (key, value) in pmod.items():
                if key == "addressBlock":
                    ab = ptag.find(key)
                    for (ab_key, ab_value) in value.items():
                        if ab.find(ab_key) is not None:
                            ab.remove(ab.find(ab_key))
                        ET.SubElement(ab, ab_key).text = str(ab_value)
                elif key == "addressBlocks":
                    for ab in ptag.findall("addressBlock"):
                        ptag.remove(ab)
                    for ab in value:
                        ab_el = ET.SubElement(ptag, "addressBlock")
                        for (ab_key, ab_value) in ab.items():
                            ET.SubElement(ab_el, ab_key).text = str(ab_value)
                else:
                    tag = ptag.find(key)
                    if tag is None:
                        tag = ET.SubElement(ptag, key)
                    tag.text = str(value)

    def add_peripheral(self, pname, padd):
        """Add pname given by padd to device."""
        parent = self.device.find("peripherals")
        for ptag in parent.iter("peripheral"):
            if ptag.find("name").text == pname:
                raise SvdPatchError("device already has a peripheral {}".format(pname))
        if "derivedFrom" in padd:
            derived = padd["derivedFrom"]
            pnew = ET.SubElement(parent, "peripheral", {"derivedFrom": derived})
        else:
            pnew = ET.SubElement(parent, "peripheral")
        ET.SubElement(pnew, "name").text = pname
        for (key, value) in padd.items():
            if key == "registers":
                ET.SubElement(pnew, "registers")
                for rname in value:
                    Peripheral(pnew).add_register(rname, value[rname])
            elif key == "interrupts":
                for iname in value:
                    Peripheral(pnew).add_interrupt(iname, value[iname])
            elif key == "addressBlock":
                ab = ET.SubElement(pnew, "addressBlock")
                for (ab_key, ab_value) in value.items():
                    ET.SubElement(ab, ab_key).text = str(ab_value)
            elif key == "addressBlocks":
                for ab in value:
                    ab_el = ET.SubElement(ptag, "addressBlock")
                    for (ab_key, ab_value) in ab.items():
                        ET.SubElement(ab_el, ab_key).text = str(ab_value)
            elif key != "derivedFrom":
                ET.SubElement(pnew, key).text = str(value)
        pnew.tail = "\n    "

    def delete_peripheral(self, pspec):
        """Delete registers matched by rspec inside ptag."""
        for ptag in list(self.iter_peripherals(pspec, check_derived=False)):
            self.device.find("peripherals").remove(ptag)

    def derive_peripheral(self, pname, pderive):
        """
        Remove registers from pname and mark it as derivedFrom pderive.
        Update all derivedFrom referencing pname.
        """
        parent = self.device.find("peripherals")
        ptag = parent.find("./peripheral[name='{}']".format(pname))
        derived = parent.find("./peripheral[name='{}']".format(pderive))
        if ptag is None:
            raise SvdPatchError("peripheral {} not found".format(pname))
        if derived is None:
            raise SvdPatchError("peripheral {} not found".format(pderive))
        for value in list(ptag):
            if value.tag in ("name", "baseAddress", "interrupt"):
                continue
            ptag.remove(value)
        for value in ptag:
            last = value
        last.tail = "\n    "
        ptag.set("derivedFrom", pderive)
        for p in parent.findall("./peripheral[@derivedFrom='{}']".format(pname)):
            p.set("derivedFrom", pderive)

    def copy_peripheral(self, pname, pmod, path):
        """
        Create copy of peripheral
        """
        parent = self.device.find("peripherals")
        ptag = parent.find("./peripheral[name='{}']".format(pname))
        pcopysrc = pmod["from"].split(":")
        pcopyname = pcopysrc[-1]
        if len(pcopysrc) == 2:
            pcopyfile = abspath(path, pcopysrc[0])
            filedev = Device(ET.parse(pcopyfile))
            source = filedev.device.find("peripherals")
        else:
            source = parent
        pcopy = copy.deepcopy(source.find("./peripheral[name='{}']".format(pcopyname)))
        if pcopy is None:
            raise SvdPatchError("peripheral {} not found".format(pcopy))

        # When copying from a peripheral in the same file, remove the
        # copied baseAddress and any interrupts.
        if source is parent:
            for value in list(pcopy):
                if value.tag in ("interrupt", "baseAddress"):
                    pcopy.remove(value)
        # Always set the name of the new peripheral to the requested name.
        pcopy.find("name").text = pname
        if ptag is not None:
            # When the target already exists, copy its baseAddress and
            # any interrupts.
            for value in list(ptag):
                if value.tag in ("interrupt", "baseAddress"):
                    pcopy.append(value)

            # Remove the original peripheral
            parent.remove(ptag)

        # Add our new copied peripheral to the device
        parent.append(pcopy)

    def rebase_peripheral(self, pnew, pold):
        """
        Move registers from pold to pnew.
        Update all derivedFrom referencing pold.
        """
        parent = self.device.find("peripherals")
        old = parent.find("./peripheral[name='{}']".format(pold))
        new = parent.find("./peripheral[name='{}']".format(pnew))
        if old is None:
            raise SvdPatchError("peripheral {} not found".format(pold))
        if new is None:
            raise SvdPatchError("peripheral {} not found".format(pnew))
        for value in new:
            last = value
        last.tail = "\n      "
        for value in list(old):
            if value.tag in ("name", "baseAddress", "interrupt"):
                continue
            old.remove(value)
            new.append(value)
        for value in old:
            last = value
        last.tail = "\n    "
        del new.attrib["derivedFrom"]
        old.set("derivedFrom", pnew)
        for p in parent.findall("./peripheral[@derivedFrom='{}']".format(pold)):
            p.set("derivedFrom", pnew)

    def clear_fields(self, pspec):
        """Clear contents of all fields inside peripherals matched by pspec"""
        for ptag in self.iter_peripherals(pspec, check_derived=False):
            p = Peripheral(ptag)
            p.clear_fields("*")

    def process_peripheral(self, pspec, peripheral, update_fields=True):
        """Work through a peripheral, handling all registers."""
        # Find all peripherals that match the spec
        pcount = 0
        for ptag in self.iter_peripherals(pspec, check_derived=False):
            pcount += 1
            p = Peripheral(ptag)

            # For derived peripherals, only process interrupts
            if "derivedFrom" in ptag.attrib:
                deletions = peripheral.get("_delete", [])
                if isinstance(deletions, dict):
                    for rspec in deletions:
                        if rspec == "_interrupts":
                            for ispec in deletions[rspec]:
                                p.delete_interrupt(ispec)
                for rspec in peripheral.get("_modify", {}):
                    if rspec == "_interrupts":
                        rmod = peripheral["_modify"][rspec]
                        for ispec in rmod:
                            p.modify_interrupt(ispec, rmod[ispec])
                for rname in peripheral.get("_add", {}):
                    if rname == "_interrupts":
                        radd = peripheral["_add"][rname]
                        for iname in radd:
                            p.add_interrupt(iname, radd[iname])
                # Don't do any further processing on derived peripherals
                continue

            # Handle deletions
            deletions = peripheral.get("_delete", [])
            if isinstance(deletions, list):
                for rspec in deletions:
                    p.delete_register(rspec)
            elif isinstance(deletions, dict):
                for rspec in deletions:
                    if rspec == "_registers":
                        for rspec in deletions[rspec]:
                            p.delete_register(rspec)
                    elif rspec == "_interrupts":
                        for ispec in deletions[rspec]:
                            p.delete_interrupt(ispec)
                    else:
                        p.delete_register(rspec)
            # Handle modifications
            for rspec in peripheral.get("_modify", {}):
                rmod = peripheral["_modify"][rspec]
                if rspec == "_registers":
                    for rspec in rmod:
                        p.modify_register(rspec, rmod[rspec])
                elif rspec == "_interrupts":
                    for ispec in rmod:
                        p.modify_interrupt(ispec, rmod[ispec])
                elif rspec == "_cluster":
                    for cspec in rmod:
                        p.modify_cluster(cspec, rmod[cspec])
                else:
                    p.modify_register(rspec, rmod)
            # Handle strips
            for prefix in peripheral.get("_strip", []):
                p.strip(prefix)
            for suffix in peripheral.get("_strip_end", []):
                p.strip(suffix, strip_end=True)
            # Handle field clearing
            for rspec in peripheral.get("_clear_fields", []):
                p.clear_fields(rspec)
            # Handle additions
            for rname in peripheral.get("_add", {}):
                radd = peripheral["_add"][rname]
                if rname == "_registers":
                    for rname in radd:
                        p.add_register(rname, radd[rname])
                elif rname == "_interrupts":
                    for iname in radd:
                        p.add_interrupt(iname, radd[iname])
                else:
                    p.add_register(rname, radd)
            for rname in peripheral.get("_derive", {}):
                rderive = peripheral["_derive"][rname]
                if rname == "_registers":
                    for rname in rderive:
                        p.derive_register(rname, rderive[rname])
                elif rname == "_interrupts":
                    raise NotImplementedError(
                        "deriving interrupts not implemented yet: {}".format(rname)
                    )
                else:
                    p.derive_register(rname, rderive)
            # Handle registers
            for rspec in peripheral:
                if not rspec.startswith("_"):
                    register = peripheral[rspec]
                    p.process_register(rspec, register, update_fields)
            # Handle register arrays
            for rspec in peripheral.get("_array", {}):
                rmod = peripheral["_array"][rspec]
                p.collect_in_array(rspec, rmod)
            # Handle clusters
            for cname in peripheral.get("_cluster", {}):
                cmod = peripheral["_cluster"][cname]
                p.collect_in_cluster(cname, cmod)
        if pcount == 0:
            raise MissingPeripheralError("Could not find {}".format(pspec))


class Peripheral:
    """Class collecting methods for processing peripheral contents"""

    def __init__(self, ptag):
        self.ptag = ptag

    def iter_registers(self, rspec):
        """
        Iterates over all registers that match rspec and live inside ptag.
        """
        for rtag in self.ptag.iter("register"):
            name = rtag.find("name").text
            if matchname(name, rspec):
                yield rtag

    def iter_registers_with_matches(self, rspec):
        """Iterates over all registers that match rspec and live inside ptag.

        Each element is a tuple of the matching register and the rspec substring
        that it matched.
        """
        for rtag in self.ptag.iter("register"):
            name = rtag.find("name").text
            if matchname(name, rspec):
                yield (rtag, matchsubspec(name, rspec))

    def iter_interrupts(self, ispec):
        """Iterates over all interrupts matching ispec"""
        for itag in self.ptag.iter("interrupt"):
            name = itag.find("name").text
            if matchname(name, ispec):
                yield itag

    def iter_clusters(self, cspec):
        """
        Iterate over all clusters that match cpsec and live inside ptag.
        """
        for ctag in self.ptag.iter("cluster"):
            name = ctag.find("name").text
            if matchname(name, cspec):
                yield ctag

    def add_interrupt(self, iname, iadd):
        """Add iname given by iadd to ptag."""
        for itag in self.ptag.iter("interrupt"):
            if itag.find("name").text == iname:
                raise SvdPatchError(
                    "peripheral {} already has an interrupt {}".format(
                        self.ptag.find("name").text, iname
                    )
                )
        inew = ET.SubElement(self.ptag, "interrupt")
        ET.SubElement(inew, "name").text = iname
        for key, val in iadd.items():
            ET.SubElement(inew, key).text = str(val)
        inew.tail = "\n    "

    def modify_interrupt(self, ispec, imod):
        """Modify ispec according to imod"""
        for itag in self.iter_interrupts(ispec):
            for (key, value) in imod.items():
                tag = itag.find(key)
                if value == "":
                    itag.remove(tag)
                else:
                    tag.text = str(value)

    def delete_interrupt(self, ispec):
        """Delete interrupts matched by ispec"""
        for itag in list(self.iter_interrupts(ispec)):
            self.ptag.remove(itag)

    def modify_register(self, rspec, rmod):
        """Modify rspec inside ptag according to rmod."""
        for rtag in self.iter_registers(rspec):
            for (key, value) in rmod.items():
                tag = rtag.find(key)
                if value == "" and tag is not None:
                    rtag.remove(tag)
                elif value != "":
                    if tag is None:
                        tag = ET.SubElement(rtag, key)
                    tag.text = str(value)

    def add_register(self, rname, radd):
        """Add rname given by radd to ptag."""
        parent = self.ptag.find("registers")
        if parent is None:
            parent = ET.SubElement(self.rtag, "registers")
        for rtag in parent.iter("register"):
            if rtag.find("name").text == rname:
                raise SvdPatchError(
                    "peripheral {} already has a register {}".format(
                        self.ptag.find("name").text, rname
                    )
                )
        rnew = ET.SubElement(parent, "register")
        ET.SubElement(rnew, "name").text = rname
        for (key, value) in radd.items():
            if key == "fields":
                ET.SubElement(rnew, "fields")
                for fname in value:
                    Register(rnew).add_field(fname, value[fname])
            else:
                ET.SubElement(rnew, key).text = str(value)
        rnew.tail = "\n        "

    def derive_register(self, rname, rderive):
        """Add rname given by deriving from rsource to ptag"""
        parent = self.ptag.find("registers")
        if not "_from" in rderive:
            raise SvdPatchError(
                "derive: source register not given, please add a _from field to {}".format(
                    rname
                )
            )
        srcname = rderive["_from"]
        source = None
        for rtag in parent.iter("register"):
            if rtag.find("name").text == rname:
                raise SvdPatchError(
                    "peripheral {} already has a register {}".format(
                        self.ptag.find("name").text, rname
                    )
                )
            if rtag.find("name").text == srcname:
                source = rtag
        if source is None:
            raise SvdPatchError(
                "peripheral {} does not have register {}".format(
                    self.ptag.find("name").text, srcname
                )
            )
        rcopy = copy.deepcopy(source)
        rcopy.find("name").text = rname
        if rcopy.find("displayName") is not None:
            rcopy.remove(rcopy.find("displayName"))
        for (key, value) in rderive.items():
            if key == "_from":
                continue
            elif key == "fields":
                raise NotImplementedError(
                    "Modifying fields in derived register not implemented"
                )
            else:
                rcopy.find(key).text = str(value)
        parent.append(rcopy)

    def delete_register(self, rspec):
        """Delete registers matched by rspec inside ptag."""
        for rtag in list(self.iter_registers(rspec)):
            self.ptag.find("registers").remove(rtag)

    def modify_cluster(self, cspec, cmod):
        """Modify cspec inside ptag according to cmod."""
        for ctag in self.iter_clusters(cspec):
            for (key, value) in cmod.items():
                tag = ctag.find(key)
                if value == "":
                    ctag.remove(tag)
                else:
                    tag.text = str(value)

    def strip(self, substr, strip_end=False):
        """
        Delete substring from register names inside ptag. Strips from the
        beginning of the name by default.
        """
        regex = create_regex_from_pattern(substr, strip_end)
        for rtag in self.ptag.iter("register"):
            nametag = rtag.find("name")
            nametag.text = regex.sub("", nametag.text)

            dnametag = rtag.find("displayName")
            if dnametag is not None:
                dnametag.text = regex.sub("", dnametag.text)

    def collect_in_array(self, rspec, rmod):
        """Collect same registers in peripheral into register array."""
        registers = []
        li, ri = spec_ind(rspec)
        for rtag in list(self.iter_registers(rspec)):
            rname = rtag.findtext("name")
            registers.append(
                [
                    rtag,
                    rname[li : len(rname) - ri],
                    int(rtag.findtext("addressOffset"), 0),
                ]
            )
        dim = len(registers)
        if dim == 0:
            raise SvdPatchError(
                "{}: registers {} not found".format(self.ptag.findtext("name"), rspec)
            )
        registers = sorted(registers, key=lambda r: r[2])

        if rmod.get("_start_from_zero"):
            dimIndex = ",".join([str(i) for i in range(dim)])
        elif dim == 1:
            dimIndex = "{0}-{0}".format(registers[0][1])
        else:
            dimIndex = ",".join(r[1] for r in registers)
        offsets = [r[2] for r in registers]
        bitmasks = [Register(r[0]).get_bitmask() for r in registers]
        dimIncrement = 0
        if dim > 1:
            dimIncrement = offsets[1] - offsets[0]

        if not (
            check_offsets(offsets, dimIncrement)
            and check_bitmasks(bitmasks, bitmasks[0])
        ):
            raise SvdPatchError(
                "{}: registers cannot be collected into {} array".format(
                    self.ptag.findtext("name"), rspec
                )
            )
        for rtag, _, _ in registers[1:]:
            self.ptag.find("registers").remove(rtag)
        rtag = registers[0][0]
        nametag = rtag.find("name")
        if "name" in rmod:
            name = rmod["name"]
        else:
            name = rspec[:li] + "%s" + rspec[len(rspec) - ri :]
        if "description" in rmod:
            desc = rmod["description"]
            if desc != "_original":
                rtag.find("description").text = desc
        elif dimIndex[0] == "0":
            desc = rtag.find("description")
            desc.text = desc.text.replace(
                nametag.text[li : len(nametag.text) - ri], "%s"
            )
        nametag.text = name
        self.process_register(name, rmod)
        ET.SubElement(rtag, "dim").text = str(dim)
        ET.SubElement(rtag, "dimIncrement").text = hex(dimIncrement)
        ET.SubElement(rtag, "dimIndex").text = dimIndex

    def collect_in_cluster(self, cname, cmod):
        """Collect registers in peripheral into clusters."""
        rdict = {}
        first = True
        check = True
        rspecs = [r for r in cmod if r != "description"]
        for rspec in rspecs:
            registers = []
            for (rtag, match_rspec) in list(self.iter_registers_with_matches(rspec)):
                rname = rtag.findtext("name")
                li, ri = spec_ind(match_rspec)
                registers.append(
                    [
                        rtag,
                        rname[li : len(rname) - ri],
                        int(rtag.findtext("addressOffset"), 0),
                    ]
                )
            registers = sorted(registers, key=lambda r: r[2])
            rdict[rspec] = registers
            bitmasks = [Register(r[0]).get_bitmask() for r in registers]
            if first:
                dim = len(registers)
                if dim == 0:
                    check = False
                    break
                dimIndex = ",".join([r[1] for r in registers])
                offsets = [r[2] for r in registers]
                dimIncrement = 0
                if dim > 1:
                    dimIncrement = offsets[1] - offsets[0]
                if not (
                    check_offsets(offsets, dimIncrement)
                    and check_bitmasks(bitmasks, bitmasks[0])
                ):
                    check = False
                    break
            else:
                if (
                    (dim != len(registers))
                    or (dimIndex != ",".join([r[1] for r in registers]))
                    or (not check_offsets(offsets, dimIncrement))
                    or (not check_bitmasks(bitmasks, bitmasks[0]))
                ):
                    check = False
                    break
            first = False
        if not check:
            raise SvdPatchError(
                "{}: registers cannot be collected into {} cluster".format(
                    self.ptag.findtext("name"), cname
                )
            )
        ctag = ET.SubElement(self.ptag.find("registers"), "cluster")
        addressOffset = min([registers[0][2] for _, registers in rdict.items()])
        ET.SubElement(ctag, "name").text = cname
        if "description" in cmod:
            description = cmod["description"]
        else:
            description = "Cluster {}, containing {}".format(cname, ", ".join(rspecs))
        ET.SubElement(ctag, "description").text = description
        ET.SubElement(ctag, "addressOffset").text = hex(addressOffset)
        for rspec, registers in rdict.items():
            for rtag, _, _ in registers[1:]:
                self.ptag.find("registers").remove(rtag)
            rtag = registers[0][0]
            rmod = cmod[rspec]
            self.process_register(rspec, rmod)
            new_rtag = copy.deepcopy(rtag)
            self.ptag.find("registers").remove(rtag)
            if "name" in rmod:
                name = rmod["name"]
            else:
                li, ri = spec_ind(rspec)
                name = rspec[:li] + rspec[len(rspec) - ri :]
            new_rtag.find("name").text = name
            if "description" in rmod:
                rtag.find("description").text = rmod["description"]
            offset = new_rtag.find("addressOffset")
            offset.text = hex(int(offset.text, 0) - addressOffset)
            ctag.append(new_rtag)
        ET.SubElement(ctag, "dim").text = str(dim)
        ET.SubElement(ctag, "dimIncrement").text = hex(dimIncrement)
        ET.SubElement(ctag, "dimIndex").text = dimIndex

    def clear_fields(self, rspec):
        """Clear contents of all fields inside registers matched by rspec"""
        for rtag in list(self.iter_registers(rspec)):
            r = Register(rtag)
            r.clear_field("*")

    def process_register(self, rspec, register, update_fields=True):
        """Work through a register, handling all fields."""
        # Find all registers that match the spec
        pname = self.ptag.find("name").text
        rcount = 0
        for rtag in self.iter_registers(rspec):
            r = Register(rtag)
            rcount += 1
            # Handle deletions
            for fspec in register.get("_delete", []):
                r.delete_field(fspec)
            # Handle field clearing
            for fspec in register.get("_clear", []):
                r.clear_field(fspec)
            # Handle modifications
            for fspec in register.get("_modify", []):
                fmod = register["_modify"][fspec]
                r.modify_field(fspec, fmod)
            # Handle additions
            for fname in register.get("_add", []):
                fadd = register["_add"][fname]
                r.add_field(fname, fadd)
            # Handle merges
            for fspec in register.get("_merge", []):
                fmerge = (
                    register["_merge"][fspec]
                    if isinstance(register["_merge"], dict)
                    else None
                )
                r.merge_fields(fspec, fmerge)
            # Handle splits
            for fspec in register.get("_split", []):
                fsplit = (
                    register["_split"][fspec]
                    if isinstance(register["_split"], dict)
                    else {}
                )
                r.split_fields(fspec, fsplit)
            # Handle strips
            for prefix in register.get("_strip", []):
                r.strip(prefix)
            for suffix in register.get("_strip_end", []):
                r.strip(suffix, strip_end=True)
            # Handle fields
            if update_fields:
                for fspec in register:
                    if not fspec.startswith("_"):
                        field = register[fspec]
                        r.process_field(pname, fspec, field)
            # Handle field arrays
            for fspec in register.get("_array", {}):
                fmod = register["_array"][fspec]
                r.collect_fields_in_array(fspec, fmod)
        if rcount == 0:
            raise MissingRegisterError("Could not find {}:{}".format(pname, rspec))


def sorted_fields(fields):
    return sorted(fields, key=lambda ftag: get_field_offset_width(ftag)[0])


class Register:
    """Class collecting methods for processing register contents"""

    def __init__(self, rtag):
        self.rtag = rtag

    def size(self):
        """
        Look up register size in bits.
        """
        size = None
        tag = self.rtag
        while size is None:
            size = tag.findtext("size")
            tag = tag.getparent()
            if tag is None:
                break
        if size is None:
            return 32
        else:
            return int(size, 0)

    def iter_fields(self, fspec):
        """
        Iterates over all fields that match fspec and live inside rtag.
        """
        fields = self.rtag.find("fields")
        if fields is not None:
            for ftag in fields.iter("field"):
                name = ftag.find("name").text
                if matchname(name, fspec):
                    yield ftag

    def strip(self, substr, strip_end=False):
        """
        Delete substring from bitfield names inside rtag. Strips from the
        beginning of the name by default.
        """
        regex = create_regex_from_pattern(substr, strip_end)
        for ftag in self.rtag.iter("field"):
            nametag = ftag.find("name")
            nametag.text = regex.sub("", nametag.text)

            dnametag = ftag.find("displayName")
            if dnametag is not None:
                dnametag.text = regex.sub("", dnametag.text)

    def modify_field(self, fspec, fmod):
        """Modify fspec inside rtag according to fmod."""
        for ftag in self.iter_fields(fspec):
            for (key, value) in fmod.items():
                if key == "_write_constraint":
                    key = "writeConstraint"

                tag = ftag.find(key)
                if tag is None:
                    tag = ET.SubElement(ftag, key)

                if key == "writeConstraint":
                    # Remove existing constraint contents
                    for child in list(tag):
                        tag.remove(child)
                    if value == "none":
                        # Completely remove the existing writeConstraint
                        ftag.remove(tag)
                    elif value == "enum":
                        # Only allow enumerated values
                        enum_tag = ET.SubElement(tag, "useEnumeratedValues")
                        enum_tag.text = "true"
                    elif isinstance(value, list):
                        # Allow a certain range
                        range_tag = make_write_constraint(value).find("range")
                        tag.append(range_tag)
                    else:
                        raise SvdPatchError(
                            "Unknown writeConstraint type {}".format(repr(value))
                        )
                else:
                    # For all other tags, just set the value
                    tag.text = str(value)

    def add_field(self, fname, fadd):
        """Add fname given by fadd to rtag."""
        parent = self.rtag.find("fields")
        if parent is None:
            parent = ET.SubElement(self.rtag, "fields")
        for ftag in parent.iter("field"):
            if ftag.find("name").text == fname:
                raise SvdPatchError(
                    "register {} already has a field {}".format(
                        self.rtag.find("name").text, fname
                    )
                )
        fnew = ET.SubElement(parent, "field")
        ET.SubElement(fnew, "name").text = fname
        for (key, value) in fadd.items():
            ET.SubElement(fnew, key).text = str(value)
        fnew.tail = "\n            "

    def delete_field(self, fspec):
        """Delete fields matched by fspec inside rtag."""
        for ftag in list(self.iter_fields(fspec)):
            self.rtag.find("fields").remove(ftag)

    def clear_field(self, fspec):
        """Clear contents of fields matched by fspec inside rtag."""
        for ftag in list(self.iter_fields(fspec)):
            for tag in ftag.findall("enumeratedValues"):
                ftag.remove(tag)
            for tag in ftag.findall("writeConstraint"):
                ftag.remove(tag)

    def merge_fields(self, key, value):
        """
        Merge all fspec in rtag.
        Support list of field to auto-merge, and dict with fspec or list of fspec
        """
        if isinstance(value, str):
            fields = list(self.iter_fields(value))
            name = key
        elif isinstance(value, list):
            fields = list()
            for fspec in value:
                fields += list(self.iter_fields(fspec))
            name = key
        elif value is not None:
            rname = self.rtag.find("name").text
            raise RegisterMergeError(
                "Invalid usage of merge for {}.{}".format(rname, key)
            )
        else:
            fields = list(self.iter_fields(key))
            name = os.path.commonprefix([f.find("name").text for f in fields])
        if len(fields) == 0:
            rname = self.rtag.find("name").text
            raise RegisterMergeError(
                "Could not find any fields to merge {}.{}".format(rname, fspec)
            )
        parent = self.rtag.find("fields")
        desc = fields[0].find("description").text
        bitwidth = sum(get_field_offset_width(f)[1] for f in fields)
        bitoffset = min(get_field_offset_width(f)[0] for f in fields)
        for field in fields:
            parent.remove(field)
        fnew = ET.SubElement(parent, "field")
        ET.SubElement(fnew, "name").text = name
        ET.SubElement(fnew, "description").text = desc
        ET.SubElement(fnew, "bitOffset").text = str(bitoffset)
        ET.SubElement(fnew, "bitWidth").text = str(bitwidth)

    def collect_fields_in_array(self, fspec, fmod):
        """Collect same fields in peripheral into register array."""
        fields = []
        li, ri = spec_ind(fspec)
        for ftag in list(self.iter_fields(fspec)):
            fname = ftag.findtext("name")
            fields.append(
                [ftag, fname[li : len(fname) - ri], get_field_offset_width(ftag)[0]]
            )
        dim = len(fields)
        if dim == 0:
            raise SvdPatchError(
                "{}: fields {} not found".format(self.rtag.findtext("name"), fspec)
            )
        fields = sorted(fields, key=lambda f: f[2])

        if fmod.get("_start_from_zero"):
            dimIndex = ",".join([str(i) for i in range(dim)])
        elif dim == 1:
            dimIndex = "{0}-{0}".format(fields[0][1])
        else:
            dimIndex = ",".join(f[1] for f in fields)
        offsets = [f[2] for f in fields]
        dimIncrement = 0
        if dim > 1:
            dimIncrement = offsets[1] - offsets[0]

        if not check_offsets(offsets, dimIncrement):
            raise SvdPatchError(
                "{}: fields cannot be collected into {} array".format(
                    self.rtag.findtext("name"), fspec
                )
            )
        for ftag, _, _ in fields[1:]:
            self.rtag.find("fields").remove(ftag)
        ftag = fields[0][0]
        nametag = ftag.find("name")
        if "name" in fmod:
            name = fmod["name"]
        else:
            name = fspec[:li] + "%s" + fspec[len(fspec) - ri :]
        if "description" in fmod:
            desc = fmod["description"]
            if desc != "_original":
                ftag.find("description").text = desc
        elif dimIndex[0] == "0":
            desc = ftag.find("description")
            desc.text = desc.text.replace(
                nametag.text[li : len(nametag.text) - ri], "%s"
            )
        nametag.text = name
        # self.process_field(name, fmod)
        ET.SubElement(ftag, "dim").text = str(dim)
        ET.SubElement(ftag, "dimIndex").text = dimIndex
        ET.SubElement(ftag, "dimIncrement").text = hex(dimIncrement)

    def split_fields(self, fspec, fsplit):
        """
        Split all fspec in rtag.
        Name and description can be customized with %s as a placeholder to the iterator value.
        """
        fields = list(self.iter_fields(fspec))
        if len(fields) == 0:
            rname = self.rtag.find("name").text
            raise RegisterMergeError(
                "Could not find any fields to split {}.{}".format(rname, fspec)
            )
        parent = self.rtag.find("fields")
        if isinstance(fsplit, dict) and "name" in fsplit:
            name = fsplit["name"]
        else:
            name = os.path.commonprefix([f.find("name").text for f in fields]) + "%s"
        if isinstance(fsplit, dict) and "description" in fsplit:
            desc = fsplit["description"]
        else:
            desc = fields[0].find("description").text
        bitoffset = get_field_offset_width(fields[0])[0]
        bitwidth = sum(get_field_offset_width(f)[1] for f in fields)
        parent.remove(fields[0])
        for i in range(bitwidth):
            fnew = ET.SubElement(parent, "field")
            ET.SubElement(fnew, "name").text = name.replace("%s", str(i))
            ET.SubElement(fnew, "description").text = desc.replace("%s", str(i))
            ET.SubElement(fnew, "bitOffset").text = str(bitoffset + i)
            ET.SubElement(fnew, "bitWidth").text = str(1)

    def process_field(self, pname, fspec, field):
        """Work through a field, handling either an enum or a range."""
        if isinstance(field, dict):
            usages = ("_read", "_write")

            if not any(u in field for u in usages):
                self.process_field_enum(pname, fspec, field)

            for usage in (u for u in usages if u in field):
                self.process_field_enum(
                    pname, fspec, field[usage], usage=usage.replace("_", "")
                )

        elif isinstance(field, list) and len(field) == 2:
            self.process_field_range(pname, fspec, field)

    def process_field_enum(self, pname, fspec, field, usage="read-write"):
        """Add an enumeratedValues given by field to all fspec in rtag."""
        replace_if_exists = False
        if "_replace_enum" in field:
            field = field["_replace_enum"]
            replace_if_exists = True

        derived, enum, enum_name, enum_usage = None, None, None, None
        for ftag in sorted_fields(list(self.iter_fields(fspec))):
            if "_derivedFrom" in field:
                derived = field["_derivedFrom"]
            else:
                name = ftag.find("name").text

            if derived is None:
                if enum is None:
                    enum = make_enumerated_values(name, field, usage=usage)
                    enum_name = enum.find("name").text
                    enum_usage = enum.find("usage").text

                for ev in ftag.iter("enumeratedValues"):
                    if len(ev) > 0:
                        ev_usage_tag = ev.find("usage")
                        ev_usage = (
                            ev_usage_tag.text
                            if ev_usage_tag is not None
                            else "read-write"
                        )
                    else:
                        # This is a derived enumeratedValues => Try to find the
                        # original definition to extract its <usage>
                        derived_name = ev.attrib["derivedFrom"]
                        derived_enums = self.rtag.findall(
                            "./fields/field/enumeratedValues/[name='{}']".format(
                                derived_name
                            )
                        )

                        if derived_enums == []:
                            raise SvdPatchError(
                                "{}: field {} derives enumeratedValues {} which could not be found".format(
                                    pname, name, derived_name
                                )
                            )
                        elif len(derived_enums) != 1:
                            raise SvdPatchError(
                                "{}: field {} derives enumeratedValues {} which was found multiple times".format(
                                    pname, name, derived_name
                                )
                            )

                        ev_usage = derived_enums[0].find("usage").text

                    if ev_usage == enum_usage or ev_usage == "read-write":
                        if replace_if_exists:
                            ftag.remove(ev)
                        else:
                            raise SvdPatchError(
                                "{}: field {} already has enumeratedValues for {}".format(
                                    pname, name, ev_usage
                                )
                            )
                ftag.append(enum)
                derived = enum_name
            else:
                ftag.append(make_derived_enumerated_values(derived))
        if derived is None:
            rname = self.rtag.find("name").text
            raise MissingFieldError(
                "Could not find {}:{}.{}".format(pname, rname, fspec)
            )

    def process_field_range(self, pname, fspec, field):
        """Add a writeConstraint range given by field to all fspec in rtag."""
        set_any = False
        for ftag in self.iter_fields(fspec):
            ftag.append(make_write_constraint(field))
            set_any = True
        if not set_any:
            rname = self.rtag.find("name").text
            raise MissingFieldError(
                "Could not find {}:{}.{}".format(pname, rname, fspec)
            )

    def get_bitmask(self):
        """Calculate filling of register"""
        mask = 0x0
        size = self.size()
        full_mask = (1 << size) - 1
        for ftag in self.iter_fields("*"):
            foffset, fwidth = get_field_offset_width(ftag)
            mask |= (full_mask >> (size - fwidth)) << foffset
        return mask


def process_device(svd, device, update_fields=True):
    """Work through a device, handling all peripherals"""

    d = Device(svd)
    # Handle any deletions
    for pspec in device.get("_delete", []):
        d.delete_peripheral(pspec)

    # Handle any copied peripherals
    for pname in device.get("_copy", {}):
        val = device["_copy"][pname]
        d.copy_peripheral(pname, val, device["_path"])

    # Handle any modifications
    for key in device.get("_modify", {}):
        val = device["_modify"][key]
        if key == "cpu":
            d.modify_cpu(val)
        elif key == "_peripherals":
            for pspec in val:
                pmod = device["_modify"]["_peripherals"][pspec]
                d.modify_peripheral(pspec, pmod)
        elif key in DEVICE_CHILDREN:
            d.modify_child(key, val)
        else:
            d.modify_peripheral(key, val)

    # Handle field clearing
    for pspec in device.get("_clear_fields", []):
        d.clear_fields(pspec)

    # Handle any new peripherals (!)
    for pname in device.get("_add", []):
        padd = device["_add"][pname]
        d.add_peripheral(pname, padd)

    # Handle any derived peripherals
    for pname in device.get("_derive", []):
        pderive = device["_derive"][pname]
        d.derive_peripheral(pname, pderive)

    # Handle any rebased peripherals
    for pname in device.get("_rebase", []):
        pold = device["_rebase"][pname]
        d.rebase_peripheral(pname, pold)

    # Now process all peripherals
    for periphspec in device:
        if not periphspec.startswith("_"):
            device[periphspec]["_path"] = device["_path"]
            d.process_peripheral(periphspec, device[periphspec], update_fields)

    # Finally apply the SVD schema sort order
    sort_recursive(svd.getroot())


def main(yaml_file):
    # Load the specified YAML root file
    with open(yaml_file) as f:
        root = yaml.safe_load(f)
        root["_path"] = yaml_file

    # Load the specified SVD file
    if "_svd" not in root:
        raise RuntimeError("You must have an svd key in the root YAML file")
    svdpath = abspath(yaml_file, root["_svd"])
    svdpath_out = svdpath + ".patched"
    svd = ET.parse(svdpath)

    # Load all included YAML files
    yaml_includes(root)

    # Process device
    process_device(svd, root)

    # SVD should now be updated, write it out
    svd.write(svdpath_out)
