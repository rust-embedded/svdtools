pub mod patch_cli;

use globset::Glob;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use svd_parser::svd::{
    addressblock::AddressBlockBuilder, interrupt::InterruptBuilder, Access, AddressBlock,
    AddressBlockUsage, ClusterInfo, ClusterInfoBuilder, Cpu, CpuBuilder, Endian, EnumeratedValue,
    EnumeratedValues, EnumeratedValuesBuilder, Field, FieldInfo, FieldInfoBuilder, Interrupt,
    PeripheralInfo, PeripheralInfoBuilder, Register, RegisterCluster, RegisterInfo,
    RegisterInfoBuilder, RegisterProperties, Usage, ValidateLevel,
};
use yaml_rust::{yaml::Hash, Yaml, YamlLoader};

mod device;
use device::DeviceExt;
mod iterators;
mod peripheral;
mod register;
mod yaml_ext;
use yaml_ext::{parse_i64, AsTypeMut, GetVal, ToYaml};

const VAL_LVL: ValidateLevel = ValidateLevel::Weak;

pub fn process_file(yaml_file: &Path) -> anyhow::Result<()> {
    // Load the specified YAML root file
    let f = File::open(yaml_file)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let mut docs = YamlLoader::load_from_str(&contents).unwrap();
    let root = docs[0].as_hash_mut().unwrap(); // select the first document
    root.insert("_path".to_yaml(), yaml_file.to_str().unwrap().to_yaml());

    // Load the specified SVD file
    let svdpath = abspath(
        &yaml_file,
        &Path::new(
            root.get_str("_svd")
                .unwrap_or_else(|| panic!("You must have an svd key in the root YAML file")),
        ),
    );
    let mut svdpath_out = svdpath.clone();
    svdpath_out.set_extension("svd.patched");
    let f = File::open(svdpath)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let mut config = svd_parser::Config::default();
    config.validate_level = ValidateLevel::Disabled;
    let mut svd = svd_parser::parse_with_config(&contents, &config)?;

    // Load all included YAML files
    yaml_includes(root)?;

    // Process device
    svd.process(root, true);

    // SVD should now be updated, write it out
    let svd_out = svd_encoder::encode(&svd)?;

    let mut f = File::create(&svdpath_out)?;
    f.write(svd_out.as_bytes())?;

    Ok(())
}

/// Gets the absolute path of relpath from the point of view of frompath.
fn abspath(frompath: &Path, relpath: &Path) -> PathBuf {
    std::fs::canonicalize(frompath.parent().unwrap().join(relpath)).unwrap()
}

/// Recursively loads any included YAML files.
pub fn yaml_includes(parent: &mut Hash) -> anyhow::Result<Vec<PathBuf>> {
    let y_path = "_path".to_yaml();
    let mut included = vec![];
    let self_path = PathBuf::from(parent.get(&y_path).unwrap().as_str().unwrap());
    let inc = parent.get_vec("_include").unwrap_or(&Vec::new()).clone();
    for relpath in inc {
        let path = abspath(&self_path, Path::new(relpath.as_str().unwrap()));
        if included.contains(&path) {
            continue;
        }
        let f = File::open(&path)?;
        let mut contents = String::new();
        (&f).read_to_string(&mut contents)?;
        let mut docs = YamlLoader::load_from_str(&contents)?;
        if docs.is_empty() {
            continue;
        }
        let child = docs[0].as_hash_mut().unwrap();
        let ypath = path.to_str().unwrap().to_yaml();
        child.insert(y_path.clone(), ypath.clone());
        included.push(path.clone());

        // Process any peripheral-level includes in child
        for (pspec, val) in child.iter_mut() {
            if !pspec.as_str().unwrap().starts_with("_") {
                match val {
                    Yaml::Hash(val) if val.contains_key(&"_include".to_yaml()) => {
                        val.insert(y_path.clone(), ypath.clone());
                        included.extend(yaml_includes(val)?);
                    }
                    _ => {}
                }
            }
        }

        // Process any top-level includes in child
        included.extend(yaml_includes(child)?);
        update_dict(parent, child)?;
    }
    Ok(included)
}

/// Recursively merge child.key into parent.key, with parent overriding
fn update_dict(parent: &mut Hash, child: &Hash) -> anyhow::Result<()> {
    use linked_hash_map::Entry;
    for (key, val) in child.iter() {
        match key {
            Yaml::String(key) if key == "_path" || key == "_include" => continue,
            key if parent.contains_key(key) => {
                if let Entry::Occupied(mut e) = parent.entry(key.clone()) {
                    match e.get_mut() {
                        el if el == val => {
                            println!("In {:?}: dublicate rule {:?}, ignored", key, val);
                        }
                        Yaml::Array(a) => match val {
                            Yaml::Array(val) => {
                                a.extend(val.clone());
                            }
                            Yaml::String(_) => {
                                if !a.contains(val) {
                                    a.push(val.clone());
                                } else {
                                    println!("In {:?}: dublicate rule {:?}, ignored", key, val);
                                }
                            }
                            _ => {}
                        },
                        Yaml::Hash(h) => {
                            update_dict(h, val.as_hash().unwrap())?;
                        }
                        s if matches!(s, Yaml::String(_)) => match val {
                            Yaml::Array(a) => {
                                if !a.contains(s) {
                                    let mut a = a.clone();
                                    a.insert(0, s.clone());
                                    e.insert(Yaml::Array(a));
                                } else {
                                    println!("In {:?}: dublicate rule {:?}, ignored", key, s);
                                }
                            }
                            s2 if matches!(s2, Yaml::String(_)) => {
                                println!(
                                    "In {:?}: conflicting rules {:?} and {:?}, ignored",
                                    key, s, s2
                                );
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            _ => {
                parent.insert(key.clone(), val.clone());
            }
        }
    }
    Ok(())
}

/// Check if name matches against a specification
fn matchname(name: &str, spec: &str) -> bool {
    matchsubspec(name, spec).is_some()
}

/// If a name matches a specification, return the first sub-specification that it matches
fn matchsubspec<'a>(name: &str, spec: &'a str) -> Option<&'a str> {
    if spec.starts_with("_") {
        return None;
    }
    if spec.contains('{') {
        let glob = Glob::new(spec).unwrap().compile_matcher();
        if glob.is_match(name) {
            return Some(spec);
        }
    } else {
        for subspec in spec.split(",") {
            let glob = Glob::new(subspec).unwrap().compile_matcher();
            if glob.is_match(name) {
                return Some(subspec);
            }
        }
    }
    return None;
}

fn modify_register_properties(p: &mut RegisterProperties, f: &str, val: &Yaml) {
    match f {
        "size" => p.size = parse_i64(val).map(|v| v as u32),
        "access" => p.access = val.as_str().and_then(Access::parse_str),
        "resetValue" => p.reset_value = parse_i64(val).map(|v| v as u64),
        "resetMask" => p.reset_mask = parse_i64(val).map(|v| v as u64),
        "protection" => {}
        _ => {}
    }
}

fn get_register_properties(h: &Hash) -> RegisterProperties {
    RegisterProperties::new()
        .size(h.get_u32("size"))
        .access(h.get_str("access").and_then(Access::parse_str))
        .reset_value(h.get_u64("resetValue"))
        .reset_mask(h.get_u64("resetMask"))
}

fn make_ev_name(name: &str, usage: Usage) -> String {
    if name.as_bytes()[0].is_ascii_digit() {
        panic!("enumeratedValue {}: can't start with a number", name);
    }
    name.to_string()
        + match usage {
            Usage::Read => "R",
            Usage::Write => "W",
            Usage::ReadWrite => "",
        }
}

fn make_ev_array(values: &Hash) -> EnumeratedValuesBuilder {
    let mut h = std::collections::BTreeMap::new();
    for (n, vd) in values {
        let vname = n.as_str().unwrap();
        if !vname.starts_with("_") {
            if vname.as_bytes()[0].is_ascii_digit() {
                panic!("enumeratedValue {} can't start with a number", vname);
            }
            let vd = vd.as_vec().unwrap();
            let value = parse_i64(&vd[0]).unwrap() as u64;
            let description = vd.get(1).and_then(Yaml::as_str).unwrap_or_else(|| {
                panic!(
                    "enumeratedValue can't have empty description for value {}",
                    value
                )
            });
            use std::collections::btree_map::Entry;
            match h.entry(value) {
                Entry::Occupied(_) => {
                    panic!("enumeratedValue can't have duplicate values")
                }
                Entry::Vacant(e) => {
                    e.insert((vname.to_string(), description.to_string()));
                }
            }
        }
    }
    EnumeratedValues::builder().values(
        h.into_iter()
            .map(|(value, vd)| {
                EnumeratedValue::builder()
                    .name(vd.0)
                    .value(Some(value))
                    .description(Some(vd.1))
                    .build(VAL_LVL)
                    .unwrap()
            })
            .collect(),
    )
}

/// Returns an enumeratedValues Element which is derivedFrom name
fn make_derived_enumerated_values(name: &str) -> EnumeratedValues {
    EnumeratedValues::builder()
        .derived_from(Some(name.into()))
        .build(VAL_LVL)
        .unwrap()
}

fn make_address_blocks(value: &Vec<Yaml>) -> Vec<AddressBlock> {
    value
        .iter()
        .map(|h| {
            make_address_block(h.as_hash().unwrap())
                .build(VAL_LVL)
                .unwrap()
        })
        .collect::<Vec<_>>()
}
fn make_address_block(h: &Hash) -> AddressBlockBuilder {
    let mut ab = AddressBlock::builder();
    if let Some(offset) = h.get_u32("offset") {
        ab = ab.offset(offset)
    }
    if let Some(size) = h.get_u32("size") {
        ab = ab.offset(size)
    }
    if let Some(usage) = h.get_str("usage").and_then(AddressBlockUsage::parse_str) {
        ab = ab.usage(usage)
    }
    ab
}

fn make_field(fadd: &Hash) -> FieldInfoBuilder {
    let mut fnew = FieldInfo::builder()
        .description(fadd.get_string("description"))
        .access(fadd.get_str("access").and_then(Access::parse_str));

    if let Some(name) = fadd.get_str("name") {
        fnew = fnew.name(name.into());
    }
    if let Some(offset) = fadd.get_i64("bitOffset") {
        fnew = fnew.bit_offset(offset as u32)
    }
    if let Some(width) = fadd.get_i64("bitWidth") {
        fnew = fnew.bit_width(width as u32)
    }

    fnew
}

fn make_register(radd: &Hash) -> RegisterInfoBuilder {
    let mut rnew = RegisterInfo::builder()
        .display_name(radd.get_string("displayName"))
        .description(radd.get_string("description"))
        .alternate_group(radd.get_string("alternateGroup"))
        .alternate_register(radd.get_string("alternateRegister"))
        .properties(get_register_properties(radd))
        .fields(radd.get_hash("fields").map(|h| {
            h.iter()
                .map(|(fname, val)| {
                    Field::Single(
                        make_field(val.as_hash().unwrap())
                            .name(fname.as_str().unwrap().into())
                            .build(VAL_LVL)
                            .unwrap(),
                    )
                })
                .collect()
        }));

    if let Some(name) = radd.get_str("name") {
        rnew = rnew.name(name.into());
    }
    if let Some(address_offset) = radd.get_i64("addressOffset") {
        rnew = rnew.address_offset(address_offset as u32);
    }
    rnew
}

fn make_cluster(cadd: &Hash) -> ClusterInfoBuilder {
    let mut cnew = ClusterInfo::builder()
        .description(cadd.get_string("description"))
        .default_register_properties(get_register_properties(cadd));

    if let Some(name) = cadd.get_str("name") {
        cnew = cnew.name(name.into());
    }
    if let Some(address_offset) = cadd.get_i64("addressOffset") {
        cnew = cnew.address_offset(address_offset as u32);
    }
    cnew
}

fn make_interrupt(iadd: &Hash) -> InterruptBuilder {
    let mut int = Interrupt::builder().description(iadd.get_string("description"));
    if let Some(name) = iadd.get_string("name") {
        int = int.name(name)
    }
    if let Some(value) = iadd.get_i64("value") {
        int = int.value(value as u32)
    }
    int
}

fn make_peripheral(padd: &Hash) -> PeripheralInfoBuilder {
    let mut pnew = PeripheralInfo::builder()
        .display_name(padd.get_string("displayName"))
        .version(padd.get_string("version"))
        .description(padd.get_string("description"))
        .group_name(padd.get_string("groupName"))
        .interrupt(padd.get_hash("interrupts").map(|value| {
            value
                .iter()
                .map(|(iname, val)| {
                    make_interrupt(val.as_hash().unwrap())
                        .name(iname.as_str().map(String::from).unwrap())
                        .build(VAL_LVL)
                        .unwrap()
                })
                .collect()
        }));
    if let Some(name) = padd.get_str("name") {
        pnew = pnew.name(name.into());
    }
    if let Some(base_address) = padd.get_i64("baseAddress") {
        pnew = pnew.base_address(base_address as u64);
    }

    if let Some(derived) = padd.get_str("derivedFrom") {
        pnew.derived_from(Some(derived.into()))
    } else {
        pnew.default_register_properties(get_register_properties(padd))
            .address_block(
                padd.get_hash("addressBlock")
                    .map(|value| vec![make_address_block(value).build(VAL_LVL).unwrap()])
                    .or_else(|| {
                        padd.get_vec("addressBlocks")
                            .map(|value| make_address_blocks(value))
                    }),
            )
            .registers(padd.get_hash("registers").map(|h| {
                h.iter()
                    .map(|(rname, val)| {
                        RegisterCluster::Register(Register::Single(
                            make_register(val.as_hash().unwrap())
                                .name(rname.as_str().unwrap().into())
                                .build(VAL_LVL)
                                .unwrap(),
                        ))
                    })
                    .collect()
            }))
    }
}

fn make_cpu(cmod: &Hash) -> CpuBuilder {
    let mut cpu = Cpu::builder()
        .fpu_double_precision(cmod.get_bool("fpuDP"))
        .dsp_present(cmod.get_bool("dspPresent"))
        .icache_present(cmod.get_bool("icachePresent"))
        .dcache_present(cmod.get_bool("dcachePresent"))
        .itcm_present(cmod.get_bool("itcmPresent"))
        .dtcm_present(cmod.get_bool("dtcmPresent"))
        .vtor_present(cmod.get_bool("vtorPresent"))
        .device_num_interrupts(cmod.get_u32("deviceNumInterrupts"))
        .sau_num_regions(cmod.get_u32("sauNumRegions"));
    if let Some(name) = cmod.get_str("name") {
        cpu = cpu.name(name.into());
    }
    if let Some(revision) = cmod.get_str("revision") {
        cpu = cpu.revision(revision.into());
    }
    if let Some(endian) = cmod.get_str("endian").and_then(Endian::parse_str) {
        cpu = cpu.endian(endian);
    }
    if let Some(mpu_present) = cmod.get_bool("mpuPresent") {
        cpu = cpu.mpu_present(mpu_present);
    }
    if let Some(fpu_present) = cmod.get_bool("fpuPresent") {
        cpu = cpu.fpu_present(fpu_present);
    }
    if let Some(nvic_priority_bits) = cmod.get_i64("nvicPrioBits") {
        cpu = cpu.nvic_priority_bits(nvic_priority_bits as u32);
    }
    if let Some(has_vendor_systick) = cmod.get_bool("vendorSystickConfig") {
        cpu = cpu.has_vendor_systick(has_vendor_systick);
    }
    cpu
}

/// Find left and right indices of enumeration token in specification string
fn spec_ind(spec: &str) -> (usize, usize) {
    let li = spec
        .bytes()
        .position(|b| b == b'*')
        .or_else(|| spec.bytes().position(|b| b == b'?'))
        .or_else(|| spec.bytes().position(|b| b == b'['))
        .unwrap();
    let ri = spec
        .bytes()
        .rev()
        .position(|b| b == b'*')
        .or_else(|| spec.bytes().rev().position(|b| b == b'?'))
        .or_else(|| spec.bytes().rev().position(|b| b == b']'))
        .unwrap();
    (li, ri)
}

fn check_offsets(offsets: &[u32], dim_increment: u32) -> bool {
    let mut it = offsets.windows(2);
    while let Some(&[o1, o2]) = it.next() {
        if o2 - o1 != dim_increment {
            return false;
        }
    }
    true
}
