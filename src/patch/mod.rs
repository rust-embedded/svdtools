#[cfg(feature = "bin")]
pub mod patch_cli;

use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use svd_parser::expand::{BlockPath, FieldPath, RegisterPath};
use svd_parser::svd::{
    addressblock::AddressBlockBuilder, interrupt::InterruptBuilder, Access, AddressBlock,
    AddressBlockUsage, ClusterInfo, ClusterInfoBuilder, Cpu, CpuBuilder, Endian, EnumeratedValue,
    EnumeratedValues, EnumeratedValuesBuilder, FieldInfo, FieldInfoBuilder, Interrupt,
    ModifiedWriteValues, PeripheralInfo, PeripheralInfoBuilder, ReadAction, RegisterCluster,
    RegisterInfo, RegisterInfoBuilder, RegisterProperties, Usage, ValidateLevel, WriteConstraint,
    WriteConstraintRange,
};
use svd_parser::SVDError::DimIndexParse;
use svd_rs::{BitRange, DimArrayIndex, DimElement, DimElementBuilder, MaybeArray};
use yaml_rust::{yaml::Hash, Yaml, YamlLoader};

use hashlink::linked_hash_map;

pub use svd_encoder::Config as EncoderConfig;

use anyhow::{anyhow, Context, Result};
pub type PatchResult = anyhow::Result<()>;

pub(crate) mod device;
use device::DeviceExt;
mod iterators;
mod peripheral;
mod register;
mod yaml_ext;
use yaml_ext::{AsType, GetVal, ToYaml};

use crate::get_encoder_config;

const VAL_LVL: ValidateLevel = ValidateLevel::Weak;

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Config {
    pub post_validate: ValidateLevel,
    pub show_patch_on_error: bool,
    pub enum_derive: EnumAutoDerive,
    pub update_fields: bool,
}

/// Derive level when several identical enumerationValues added in a field
#[cfg_attr(feature = "bin", derive(clap::ValueEnum))]
#[cfg_attr(feature = "bin", value(rename_all = "lower"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnumAutoDerive {
    #[default]
    /// Derive enumeratedValues
    Enum,
    /// Derive fields
    Field,
    /// Make a copy
    None,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            post_validate: ValidateLevel::Disabled,
            show_patch_on_error: false,
            enum_derive: Default::default(),
            update_fields: true,
        }
    }
}

pub fn load_patch(yaml_file: &Path) -> Result<Yaml> {
    // Load the specified YAML root file
    let f = File::open(yaml_file)?;
    let mut contents = String::new();
    (&f).read_to_string(&mut contents)?;
    let docs = YamlLoader::load_from_str(&contents)?;
    let mut doc = docs.into_iter().next().unwrap(); // select the first document
    let root = doc.hash_mut()?;
    root.insert("_path".to_yaml(), yaml_file.to_str().unwrap().to_yaml());

    // Load all included YAML files
    yaml_includes(root)?;
    Ok(doc)
}

pub fn process_file(
    yaml_file: &Path,
    out_path: Option<&Path>,
    format_config: Option<&Path>,
    config: &Config,
) -> Result<()> {
    let doc = load_patch(yaml_file)?;

    // Load the specified SVD file
    let svdpath = abspath(
        yaml_file,
        Path::new(
            doc.hash()?
                .get_str("_svd")?
                .ok_or_else(|| anyhow!("You must have an svd key in the root YAML file"))?,
        ),
    )?;
    let svdpath_out = if let Some(out_path) = out_path {
        out_path.to_owned()
    } else {
        let mut pth = svdpath.clone();
        pth.set_extension("svd.patched");
        pth
    };

    let encoder_config = get_encoder_config(format_config)?;

    let mut svd_out = process_reader(File::open(svdpath)?, &doc, &encoder_config, config)?;
    std::io::copy(&mut svd_out, &mut File::create(svdpath_out)?)?;

    Ok(())
}

pub fn process_reader<R: Read>(
    mut svd: R,
    patch: &Yaml,
    format_config: &EncoderConfig,
    config: &Config,
) -> Result<impl Read> {
    let mut contents = String::new();
    svd.read_to_string(&mut contents)?;
    let mut parser_config = svd_parser::Config::default();
    parser_config.validate_level = ValidateLevel::Disabled;
    let mut dev = svd_parser::parse_with_config(&contents, &parser_config)?;

    // Process device
    dev.process(patch.hash()?, config).with_context(|| {
        let name = &dev.name;
        let mut out_str = String::new();
        let mut emitter = yaml_rust::YamlEmitter::new(&mut out_str);
        emitter.dump(patch).unwrap();
        if config.show_patch_on_error {
            format!("Processing device `{name}`. Patches looks like:\n{out_str}")
        } else {
            format!("Processing device `{name}`")
        }
    })?;

    dev.validate_all(config.post_validate)?;

    Ok(Cursor::new(
        svd_encoder::encode_with_config(&dev, format_config)?.into_bytes(),
    ))
}

/// Gets the absolute path of relpath from the point of view of frompath.
fn abspath(frompath: &Path, relpath: &Path) -> Result<PathBuf, std::io::Error> {
    normpath::BasePath::new(frompath)
        .unwrap()
        .parent()
        .unwrap()
        .unwrap()
        .join(relpath)
        .canonicalize()
        .map(|b| b.as_path().into())
}

/// Recursively loads any included YAML files.
pub fn yaml_includes(parent: &mut Hash) -> Result<Vec<PathBuf>> {
    let y_path = "_path".to_yaml();
    let mut included = vec![];
    let self_path = PathBuf::from(parent.get(&y_path).unwrap().str()?);

    // Process any peripheral-level includes in child
    for (pspec, val) in parent.iter_mut() {
        if !pspec.str()?.starts_with('_') {
            match val {
                Yaml::Hash(val) if val.contains_key(&"_include".to_yaml()) => {
                    let ypath = self_path.to_str().unwrap().to_yaml();
                    val.insert(y_path.clone(), ypath.clone());
                    included.extend(yaml_includes(val)?);
                }
                _ => {}
            }
        }
    }

    let inc = parent
        .str_vec_iter("_include")?
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    for relpath in inc {
        let relpath = relpath.as_str();
        let path = abspath(&self_path, Path::new(relpath))
            .with_context(|| anyhow!("Opening file \"{relpath}\" from file {self_path:?}"))?;
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
        let child = docs[0].hash_mut()?;
        let ypath = path.to_str().unwrap().to_yaml();
        child.insert(y_path.clone(), ypath.clone());
        included.push(path.clone());

        // Process any top-level includes in child
        included.extend(yaml_includes(child)?);
        update_dict(parent, child)?;
    }
    parent.remove(&"_include".to_yaml());
    Ok(included)
}

/// Recursively merge child.key into parent.key, with parent overriding
fn update_dict(parent: &mut Hash, child: &Hash) -> Result<()> {
    use linked_hash_map::Entry;
    for (key, val) in child.iter() {
        match key {
            Yaml::String(key) if key == "_path" || key == "_include" => continue,
            Yaml::String(k) if parent.contains_key(key) && k.starts_with('_') => {
                if let Entry::Occupied(mut e) = parent.entry(key.clone()) {
                    match e.get_mut() {
                        el if el == val => {
                            println!("In {k}: dublicate rule {val:?}, ignored");
                        }
                        Yaml::Array(a) => match val {
                            Yaml::Array(val) => {
                                a.extend(val.clone());
                            }
                            Yaml::String(_) => {
                                if !a.contains(val) {
                                    a.push(val.clone());
                                } else {
                                    println!("In {k}: dublicate rule {val:?}, ignored");
                                }
                            }
                            _ => {}
                        },
                        Yaml::Hash(h) => {
                            update_dict(h, val.hash()?)?;
                        }
                        s if matches!(s, Yaml::String(_)) => match val {
                            Yaml::Array(a) => {
                                if !a.contains(s) {
                                    let mut a = a.clone();
                                    a.insert(0, s.clone());
                                    e.insert(Yaml::Array(a));
                                } else {
                                    println!("In {k}: dublicate rule {s:?}, ignored");
                                }
                            }
                            s2 if matches!(s2, Yaml::String(_)) => {
                                println!("In {k}: conflicting rules {s:?} and {s2:?}, ignored");
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            Yaml::String(_) if parent.contains_key(key) => {
                let mut i = 0;
                loop {
                    let key = Yaml::Array(vec![key.clone(), Yaml::Integer(i)]);
                    if !parent.contains_key(&key) {
                        parent.insert(key, val.clone());
                        break;
                    }
                    i += 1;
                }
            }
            Yaml::Array(a)
                if parent.contains_key(key)
                    && matches!(a.as_slice(), [Yaml::String(_), Yaml::Integer(_)]) =>
            {
                if let [k @ Yaml::String(_), Yaml::Integer(_)] = a.as_slice() {
                    let mut i = 0;
                    loop {
                        let key = Yaml::Array(vec![k.clone(), Yaml::Integer(i)]);
                        if !parent.contains_key(&key) {
                            parent.insert(key, val.clone());
                            break;
                        }
                        i += 1;
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

fn newglob(spec: &str) -> globset::GlobMatcher {
    globset::GlobBuilder::new(spec)
        .backslash_escape(true)
        .build()
        .unwrap()
        .compile_matcher()
}

/// If a name matches a specification, return the first sub-specification that it matches
fn matchsubspec<'a>(name: &str, spec: &'a str) -> Option<&'a str> {
    if spec.contains('{') {
        let glob = newglob(spec);
        if glob.is_match(name) {
            return Some(spec);
        }
    } else {
        for subspec in spec.split(',') {
            let glob = newglob(subspec);
            if glob.is_match(name) {
                return Some(subspec);
            }
        }
    }
    None
}

fn modify_register_properties(p: &mut RegisterProperties, f: &str, val: &Yaml) -> PatchResult {
    match f {
        "size" => p.size = Some(val.i64()? as u32),
        "access" => p.access = Access::parse_str(val.str()?),
        "resetValue" => p.reset_value = Some(val.i64()? as u64),
        "resetMask" => p.reset_mask = Some(val.i64()? as u64),
        "protection" => {}
        _ => {}
    }
    Ok(())
}

fn get_register_properties(h: &Hash) -> Result<RegisterProperties> {
    Ok(RegisterProperties::new()
        .size(h.get_u32("size")?)
        .access(h.get_str("access")?.and_then(Access::parse_str))
        .reset_value(h.get_u64("resetValue")?)
        .reset_mask(h.get_u64("resetMask")?))
}

fn make_ev_name(name: &str, usage: Option<Usage>) -> Result<String> {
    if name.as_bytes()[0].is_ascii_digit() {
        return Err(anyhow!(
            "enumeratedValue {}: can't start with a number",
            name
        ));
    }
    Ok(name.to_string()
        + match usage.unwrap_or_default() {
            Usage::Read => "R",
            Usage::Write => "W",
            Usage::ReadWrite => "",
        })
}

fn make_ev_array(values: &Hash) -> Result<EnumeratedValuesBuilder> {
    let mut h = std::collections::BTreeMap::new();
    for (n, vd) in values {
        let vname = n.str()?;
        if !vname.starts_with('_') {
            if vname.as_bytes()[0].is_ascii_digit() {
                return Err(anyhow!(
                    "enumeratedValue {} can't start with a number",
                    vname
                ));
            }
            let vd = vd.vec()?;
            let Some((value, description)) = vd.first().zip(vd.get(1)) else {
                return Err(anyhow!(
                    "enumeratedValue {vname} can't have empty value or description"
                ));
            };
            let value = value.i64()?;
            let description = description.str()?;
            let def = value == -1;
            let value = value as u64;
            let ev = EnumeratedValue::builder()
                .name(vname.into())
                .description(Some(description.into()));
            let ev = (if def {
                ev.is_default(Some(true))
            } else {
                ev.value(Some(value))
            })
            .build(VAL_LVL)?;
            use std::collections::btree_map::Entry;
            match h.entry(value) {
                Entry::Occupied(_) => {
                    return Err(anyhow!("enumeratedValue can't have duplicate values"));
                }
                Entry::Vacant(e) => {
                    e.insert(ev);
                }
            }
        }
    }
    Ok(EnumeratedValues::builder().values(h.into_values().collect()))
}

/// Returns an enumeratedValues Element which is derivedFrom name
fn make_derived_enumerated_values(name: &str) -> Result<EnumeratedValues> {
    Ok(EnumeratedValues::builder()
        .derived_from(Some(name.into()))
        .build(VAL_LVL)?)
}

fn make_address_blocks(value: &[Yaml]) -> Result<Vec<AddressBlock>> {
    let mut blocks = Vec::new();
    for h in value {
        blocks.push(make_address_block(h.hash()?)?.build(VAL_LVL)?);
    }
    Ok(blocks)
}
fn make_address_block(h: &Hash) -> Result<AddressBlockBuilder> {
    let mut ab = AddressBlock::builder();
    if let Some(offset) = h.get_u32("offset")? {
        ab = ab.offset(offset)
    }
    if let Some(size) = h.get_u32("size")? {
        ab = ab.size(size)
    }
    if let Some(usage) = h.get_str("usage")?.and_then(AddressBlockUsage::parse_str) {
        ab = ab.usage(usage)
    }
    Ok(ab)
}

fn make_dim_element(h: &Hash) -> Result<Option<DimElementBuilder>> {
    let mut d = DimElement::builder()
        .dim_index(if let Some(y) = h.get_yaml("dimIndex") {
            match y {
                Yaml::String(text) => Some(DimElement::parse_indexes(text).ok_or(DimIndexParse)?),
                Yaml::Array(a) => {
                    let mut v = Vec::new();
                    for s in a {
                        v.push(s.as_str().ok_or(DimIndexParse)?.to_string());
                    }
                    Some(v)
                }
                _ => return Err(DimIndexParse.into()),
            }
        } else {
            None
        })
        .dim_name(h.get_string("dimName")?)
        .dim_array_index(
            h.get_hash("dimArrayIndex")?
                .map(make_dim_array_index)
                .transpose()?,
        );
    if let Some(dim) = h.get_u32("dim")? {
        d = d.dim(dim)
    }
    if let Some(dim_increment) = h.get_u32("dimIncrement")? {
        d = d.dim_increment(dim_increment)
    }
    Ok(if d == DimElement::builder() {
        None
    } else {
        Some(d)
    })
}

fn make_dim_array_index(h: &Hash) -> Result<DimArrayIndex> {
    let mut values = Vec::new();
    for (vname, vd) in h {
        let vname = vname.str()?;
        if let Some(vd) = vd.as_vec() {
            let Some((value, description)) = vd.first().zip(vd.get(1)) else {
                return Err(anyhow!(
                    "enumeratedValue {vname} can't have empty value or description"
                ));
            };
            let value = value.i64()?;
            let description = description.str()?;
            values.push(
                EnumeratedValue::builder()
                    .value(Some(value as u64))
                    .description(Some(description.into()))
                    .build(VAL_LVL)?,
            )
        }
    }

    Ok(DimArrayIndex {
        header_enum_name: h.get_string("headerEnumName")?,
        values,
    })
}

fn modify_dim_element<T: Clone>(
    tag: &mut MaybeArray<T>,
    dim: &Option<DimElementBuilder>,
) -> PatchResult {
    if let Some(dim) = dim.as_ref() {
        match tag {
            MaybeArray::Array(_, array_info) => array_info.modify_from(dim.clone(), VAL_LVL)?,
            MaybeArray::Single(info) => {
                let array_info = dim.clone().build(VAL_LVL)?;
                *tag = MaybeArray::Array(info.clone(), array_info);
            }
        }
    }
    Ok(())
}

fn make_field(fadd: &Hash, rpath: Option<&RegisterPath>) -> Result<FieldInfoBuilder> {
    let mut fnew = FieldInfo::builder()
        .description(opt_interpolate(&rpath, fadd.get_str("description")?))
        .derived_from(opt_interpolate(&rpath, fadd.get_str("derivedFrom")?))
        .access(fadd.get_str("access")?.and_then(Access::parse_str))
        .modified_write_values(
            fadd.get_str("modifiedWriteValues")?
                .and_then(ModifiedWriteValues::parse_str),
        )
        .read_action(fadd.get_str("readAction")?.and_then(ReadAction::parse_str))
        .write_constraint(get_write_constraint(fadd)?);

    if let Some(name) = fadd.get_str("name")? {
        fnew = fnew.name(name.into());
    }
    // NOTE: support only both `msb` and `lsb` passed together
    if let (Some(msb), Some(lsb)) = (fadd.get_i64("msb")?, fadd.get_i64("lsb")?) {
        fnew = fnew.bit_range(BitRange::from_msb_lsb(msb as _, lsb as _));
    } else if let Some(bit_range) = fadd.get_str("bitRange")?.and_then(BitRange::from_bit_range) {
        fnew = fnew.bit_range(bit_range);
    } else {
        if let Some(offset) = fadd.get_i64("bitOffset")? {
            fnew = fnew.bit_offset(offset as u32)
        }
        if let Some(width) = fadd.get_i64("bitWidth")? {
            fnew = fnew.bit_width(width as u32)
        }
    }

    Ok(fnew)
}

fn make_register(radd: &Hash, path: Option<&BlockPath>) -> Result<RegisterInfoBuilder> {
    let mut rnew = RegisterInfo::builder()
        .display_name(radd.get_string("displayName")?)
        .description(opt_interpolate(&path, radd.get_str("description")?))
        .derived_from(opt_interpolate(&path, radd.get_str("derivedFrom")?))
        .alternate_group(radd.get_string("alternateGroup")?)
        .alternate_register(radd.get_string("alternateRegister")?)
        .properties(get_register_properties(radd)?)
        .write_constraint(get_write_constraint(radd)?)
        .fields(match radd.get_hash("fields")? {
            Some(h) => {
                let mut fields = Vec::new();
                for (fname, val) in h {
                    fields.push({
                        let fadd = val.hash()?;
                        let field = make_field(fadd, None)?
                            .name(fname.str()?.into())
                            .build(VAL_LVL)?;
                        if let Some(dim) = make_dim_element(fadd)? {
                            field.array(dim.build(VAL_LVL)?)
                        } else {
                            field.single()
                        }
                    });
                }
                Some(fields)
            }
            _ => None,
        });

    if let Some(name) = radd.get_str("name")? {
        rnew = rnew.name(name.into());
    }
    if let Some(address_offset) = radd.get_i64("addressOffset")? {
        rnew = rnew.address_offset(address_offset as u32);
    }

    Ok(rnew)
}

fn get_write_constraint(h: &Hash) -> Result<Option<WriteConstraint>> {
    if let Some(write_constraint) = h
        .get_yaml("_write_constraint")
        .or_else(|| h.get_yaml("writeConstraint"))
    {
        match write_constraint {
            Yaml::String(s) if s == "none" => {
                // Completely remove the existing writeConstraint
                Ok(None)
            }
            Yaml::String(s) if s == "enum" => {
                // Only allow enumerated values
                Ok(Some(WriteConstraint::UseEnumeratedValues(true)))
            }
            Yaml::Array(a) => {
                // Allow a certain range
                Ok(Some(WriteConstraint::Range(WriteConstraintRange {
                    min: a[0].i64()? as u64,
                    max: a[1].i64()? as u64,
                })))
            }
            _ => Err(anyhow!("Unknown writeConstraint type {write_constraint:?}")),
        }
    } else {
        Ok(None)
    }
}

fn make_cluster(cadd: &Hash, path: Option<&BlockPath>) -> Result<ClusterInfoBuilder> {
    let mut cnew = ClusterInfo::builder()
        .description(opt_interpolate(&path, cadd.get_str("description")?))
        .derived_from(opt_interpolate(&path, cadd.get_str("derivedFrom")?))
        .default_register_properties(get_register_properties(cadd)?);

    if let Some(h) = cadd.get_hash("registers")? {
        let mut ch = Vec::new();
        for (rname, val) in h {
            ch.push(RegisterCluster::Register({
                let radd = val.hash()?;
                let reg = make_register(radd, None)?
                    .name(rname.str()?.into())
                    .build(VAL_LVL)?;
                if let Some(dim) = make_dim_element(radd)? {
                    reg.array(dim.build(VAL_LVL)?)
                } else {
                    reg.single()
                }
            }));
        }
        cnew = cnew.children(ch);
    }

    if let Some(name) = cadd.get_str("name")? {
        cnew = cnew.name(name.into());
    }
    if let Some(address_offset) = cadd.get_i64("addressOffset")? {
        cnew = cnew.address_offset(address_offset as u32);
    }
    Ok(cnew)
}

fn make_interrupt(iadd: &Hash) -> Result<InterruptBuilder> {
    let mut int = Interrupt::builder().description(iadd.get_string("description")?);
    if let Some(name) = iadd.get_string("name")? {
        int = int.name(name)
    }
    if let Some(value) = iadd.get_i64("value")? {
        int = int.value(value as u32)
    }
    Ok(int)
}

fn make_peripheral(padd: &Hash, modify: bool) -> Result<PeripheralInfoBuilder> {
    let mut pnew = PeripheralInfo::builder()
        .display_name(padd.get_string("displayName")?)
        .version(padd.get_string("version")?)
        .description(padd.get_string("description")?)
        .derived_from(padd.get_string("derivedFrom")?)
        .group_name(padd.get_string("groupName")?)
        .interrupt(if !modify {
            match padd.get_hash("interrupts")? {
                Some(h) => {
                    let mut interupts = Vec::new();
                    for (iname, val) in h {
                        interupts.push(
                            make_interrupt(val.hash()?)?
                                .name(iname.str()?.into())
                                .build(VAL_LVL)?,
                        );
                    }
                    Some(interupts)
                }
                _ => None,
            }
        } else {
            None
        });
    if let Some(name) = padd.get_str("name")? {
        pnew = pnew.name(name.into());
    }
    if let Some(base_address) = padd.get_i64("baseAddress")? {
        pnew = pnew.base_address(base_address as u64);
    }

    if let Some(derived) = padd.get_str("derivedFrom")? {
        Ok(pnew.derived_from(Some(derived.into())))
    } else {
        Ok(pnew
            .default_register_properties(get_register_properties(padd)?)
            .address_block(if !modify {
                if let Some(h) = padd.get_hash("addressBlock").ok().flatten() {
                    Some(vec![make_address_block(h)?.build(VAL_LVL)?])
                } else if let Some(h) = padd.get_vec("addressBlocks").ok().flatten() {
                    Some(make_address_blocks(h)?)
                } else {
                    None
                }
            } else {
                None
            })
            .registers(match padd.get_hash("registers")? {
                Some(h) => {
                    let mut regs = Vec::new();
                    for (rname, val) in h.iter() {
                        regs.push(RegisterCluster::Register({
                            let radd = val.hash()?;
                            let reg = make_register(radd, None)?
                                .name(rname.str()?.into())
                                .build(VAL_LVL)?;
                            if let Some(dim) = make_dim_element(radd)? {
                                reg.array(dim.build(VAL_LVL)?)
                            } else {
                                reg.single()
                            }
                        }));
                    }
                    Some(regs)
                }
                _ => None,
            }))
    }
}

fn make_cpu(cmod: &Hash) -> Result<CpuBuilder> {
    let mut cpu = Cpu::builder()
        .fpu_double_precision(cmod.get_bool("fpuDP")?)
        .dsp_present(cmod.get_bool("dspPresent")?)
        .icache_present(cmod.get_bool("icachePresent")?)
        .dcache_present(cmod.get_bool("dcachePresent")?)
        .itcm_present(cmod.get_bool("itcmPresent")?)
        .dtcm_present(cmod.get_bool("dtcmPresent")?)
        .vtor_present(cmod.get_bool("vtorPresent")?)
        .device_num_interrupts(cmod.get_u32("deviceNumInterrupts")?)
        .sau_num_regions(cmod.get_u32("sauNumRegions")?);
    if let Some(name) = cmod.get_string("name")? {
        cpu = cpu.name(name);
    }
    if let Some(revision) = cmod.get_string("revision")? {
        cpu = cpu.revision(revision);
    }
    if let Some(endian) = cmod.get_str("endian")?.and_then(Endian::parse_str) {
        cpu = cpu.endian(endian);
    }
    if let Some(mpu_present) = cmod.get_bool("mpuPresent")? {
        cpu = cpu.mpu_present(mpu_present);
    }
    if let Some(fpu_present) = cmod.get_bool("fpuPresent")? {
        cpu = cpu.fpu_present(fpu_present);
    }
    if let Some(nvic_priority_bits) = cmod.get_i64("nvicPrioBits")? {
        cpu = cpu.nvic_priority_bits(nvic_priority_bits as u32);
    }
    if let Some(has_vendor_systick) = cmod.get_bool("vendorSystickConfig")? {
        cpu = cpu.has_vendor_systick(has_vendor_systick);
    }
    Ok(cpu)
}

/// Find left and right indices of enumeration token in specification string
///
/// # Examples
///
/// ```ignore
/// let cases = [
///     ("RELOAD?", (6, 0)),
///     ("TMR[1-57]_MUX", (3, 4)),
///     ("DT[1-3]?", (2, 0)),
///     ("GPIO[ABCDE]", (4, 0)),
///     ("CSPT[1][7-9],CSPT[2][0-5]", (4, 0)),
/// ];
/// for (spec, (li, ri)) in cases {
///     assert_eq!(spec_ind(spec), Some((li, ri)));
/// }
/// ```
///
fn spec_ind(spec: &str) -> Option<(usize, usize)> {
    use once_cell::sync::Lazy;
    use regex::Regex;
    let spec = spec.split(',').next().unwrap_or(spec);
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[\w%]*((?:[\?*]|\[\d+(?:-\d+)?\]|\[[a-zA-Z]+(?:-[a-zA-Z]+)?\])+)[\w%]*$")
            .unwrap()
    });
    let caps = RE.captures(spec)?;
    let spec = caps.get(0).unwrap();
    let token = caps.get(1).unwrap();
    let li = token.start();
    let ri = spec.end() - token.end();
    Some((li, ri))
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

/// Tries to get common description (or displayNames) for register/field array with "%s" in index position.
/// Returns `None` if incoming descriptions have more then 1 difference
fn common_description(descs: &[Option<&str>], dim_index: &[String]) -> Option<Option<String>> {
    if let Some(desc0) = descs[0] {
        let idx0 = &dim_index[0];
        if desc0.contains(idx0) {
            for (i1, _) in desc0.match_indices(idx0) {
                let (s1, sx) = desc0.split_at(i1);
                let (_, s2) = sx.split_at(idx0.len());
                let dsc = Some(format!("{s1}%s{s2}"));
                let mut same = true;
                for (d, idx) in descs.iter().zip(dim_index).skip(1) {
                    if d != &dsc
                        .as_ref()
                        .map(|dsc| dsc.replacen("%s", idx, 1))
                        .as_deref()
                    {
                        same = false;
                        break;
                    }
                }
                if same {
                    return Some(dsc);
                }
            }
        }
    }
    // If descriptions are identical, do not change.
    let desc0 = &descs[0];
    let mut same = true;
    for d in &descs[1..] {
        if d != desc0 {
            same = false;
            break;
        }
    }
    same.then(|| desc0.map(Into::into))
}

pub trait Spec {
    /// Return specification and `ignore_if_not_exists` flag
    fn spec(&self) -> (&str, bool);
}

impl Spec for str {
    fn spec(&self) -> (&str, bool) {
        if let Some(s) = self.strip_prefix("?~") {
            (s, true)
        } else {
            (self, false)
        }
    }
}

pub(crate) fn check_dimable_name(name: &str) -> Result<()> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| {
        Regex::new("^(((%s)|(%s)[_A-Za-z]{1}[_A-Za-z0-9]*)|([_A-Za-z]{1}[_A-Za-z0-9]*(\\[%s\\])?)|([_A-Za-z]{1}[_A-Za-z0-9]*(%s)?[_A-Za-z0-9]*))$").unwrap()
    });
    if PATTERN.is_match(name) {
        Ok(())
    } else {
        Err(anyhow!("`{name}` is incorrect name"))
    }
}

fn opt_interpolate<T: Interpolate>(path: &Option<&T>, s: Option<&str>) -> Option<String> {
    if let Some(path) = path {
        path.interpolate_opt(s)
    } else {
        s.map(ToString::to_string)
    }
}

fn adding_pos<'a, T, U: Eq + Ord>(
    new: &'a T,
    iter: impl IntoIterator<Item = &'a T>,
    f: impl Fn(&'a T) -> U,
) -> usize {
    let address = f(new);
    iter.into_iter()
        .enumerate()
        .filter(|(_, p)| f(p) <= address)
        .max_by_key(|(_, p)| f(p))
        .map(|(i, _)| i + 1)
        .unwrap_or(0)
}

trait Interpolate {
    fn interpolate<'a>(&self, s: &'a str) -> Cow<'a, str>;
    fn interpolate_opt(&self, s: Option<&str>) -> Option<String> {
        s.map(|s| self.interpolate(s).into_owned())
    }
}

impl Interpolate for BlockPath {
    fn interpolate<'a>(&self, s: &'a str) -> Cow<'a, str> {
        let mut cow = Cow::Borrowed(s);
        if cow.contains("`peripheral`") {
            cow = cow.replace("`peripheral`", &self.peripheral).into()
        }
        if cow.contains("`block_path`") {
            cow = cow.replace("`block_path`", &self.to_string()).into()
        }
        cow
    }
}

impl Interpolate for RegisterPath {
    fn interpolate<'a>(&self, s: &'a str) -> Cow<'a, str> {
        let mut cow = self.block.interpolate(s);
        if cow.contains("`register`") {
            cow = cow.replace("`register`", &self.name).into()
        }
        if cow.contains("`register_path`") {
            cow = cow.replace("`register_path`", &self.to_string()).into()
        }
        cow
    }
}

impl Interpolate for FieldPath {
    fn interpolate<'a>(&self, s: &'a str) -> Cow<'a, str> {
        let mut cow = self.register().interpolate(s);
        if cow.contains("`field`") {
            cow = cow.replace("`field`", &self.name).into()
        }
        if cow.contains("`field_path`") {
            cow = cow.replace("`field_path`", &self.to_string()).into()
        }
        cow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::path::Path;

    #[test]
    fn add_register() -> Result<()> {
        let (mut device, yaml) = test_utils::get_patcher(Path::new("add_register")).unwrap();
        assert_eq!(device.peripherals.len(), 1);
        let registers = device.peripherals[0]
            .registers
            .as_ref()
            .ok_or(anyhow!("no registers"))?;
        assert_eq!(registers.len(), 1);

        device.process(&yaml, &Default::default()).unwrap();
        assert_eq!(device.peripherals.len(), 1);
        let periph1 = &device.peripherals[0];
        assert_eq!(periph1.name, "DAC1");
        let registers = device.peripherals[0]
            .registers
            .as_ref()
            .ok_or(anyhow!("no registers"))?;
        assert_eq!(registers.len(), 2);
        let reg2 = match &registers[1] {
            RegisterCluster::Register(r) => r,
            RegisterCluster::Cluster(_) => return Err(anyhow!("expected register, found cluster")),
        };
        assert_eq!(reg2.name, "ANOTHER_REG");
        assert_eq!(reg2.address_offset, 4);
        assert_eq!(reg2.properties.size, Some(32));
        assert_eq!(reg2.write_constraint, None);
        let fields = reg2.fields.as_ref().ok_or(anyhow!("no fields"))?;
        assert_eq!(fields.len(), 1);
        let field1 = &fields[0];
        assert_eq!(field1.name, "MPS");
        assert_eq!(field1.bit_offset(), 0);
        assert_eq!(field1.bit_width(), 5);
        assert_eq!(field1.access, Some(Access::ReadWrite));
        assert_eq!(
            field1.write_constraint,
            Some(WriteConstraint::Range(WriteConstraintRange {
                min: 0,
                max: 0x1f
            }))
        );

        Ok(())
    }
}
