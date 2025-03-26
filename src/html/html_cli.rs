use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(target_os = "macos")]
use std::os::macos::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use svd_parser::svd::{BitRange, Field};

use anyhow::{anyhow, Context};
use liquid::{
    model::{object, Scalar},
    Object,
};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use svd_parser::expand::{
    derive_cluster, derive_enumerated_values, derive_field, derive_register, BlockPath,
    RegisterPath,
};
use svd_parser::{
    expand::{derive_peripheral, Index},
    svd::{Access, Cluster, Register, RegisterInfo, WriteConstraint},
};
use svd_rs::{EnumeratedValue, EnumeratedValues, ModifiedWriteValues, ReadAction};

fn sanitize(input: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*\n\s*").unwrap());
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("[<>&]").unwrap());

    let s = RE.replace_all(input, " ");
    REGEX
        .replace_all(&s, |caps: &regex::Captures| {
            match caps.get(0).unwrap().as_str() {
                "<" => "&lt;".to_owned(),
                ">" => "&gt;".to_owned(),
                "&" => "&amp;".to_owned(),
                _ => unreachable!(),
            }
        })
        .into()
}

fn hex(val: u64) -> String {
    format!("0x{val:x}")
}

fn progress(documented: i64, total: i64) -> String {
    let f = if total > 0 {
        100. * (documented as f64 / total as f64)
    } else {
        100.
    };
    if f.round() == f {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

fn generate_index_page(devices: &Vec<Object>, writer: &mut dyn Write) -> anyhow::Result<()> {
    println!("Generating Index");
    let template_file = include_str!("index.template.html");
    let template = liquid::ParserBuilder::with_stdlib()
        .build()
        .unwrap()
        .parse(template_file)
        .unwrap();
    let globals = liquid::object!({ "devices": devices });
    template.render_to(writer, &globals)?;
    Ok(())
}

fn generate_device_page(
    template: &liquid::Template,
    device: &Object,
    writer: &mut dyn Write,
) -> anyhow::Result<()> {
    let globals = liquid::object!({ "device": device });
    template.render_to(writer, &globals)?;
    Ok(())
}

fn short_access(accs: &str) -> &str {
    match accs {
        "read-write" => "rw",
        "read-only" => "r",
        "write-only" => "w",
        _ => "N/A",
    }
}

fn short_mwv(mwv: ModifiedWriteValues) -> &'static str {
    use ModifiedWriteValues::*;
    match mwv {
        OneToClear => "w1c",
        OneToSet => "w1s",
        OneToToggle => "w1t",
        ZeroToClear => "w0c",
        ZeroToSet => "w0s",
        ZeroToToggle => "w0t",
        Clear => "wc",
        Set => "ws",
        Modify => "w",
    }
}

fn short_ra(ra: Option<ReadAction>) -> &'static str {
    match ra {
        None => "r",
        Some(ReadAction::Clear) => "rc",
        Some(ReadAction::Set) => "rs",
        Some(ReadAction::Modify) => "rm",
        Some(ReadAction::ModifyExternal) => "re",
    }
}

trait GetI64 {
    fn get_i64(&self, key: &str) -> Option<i64>;
    fn get_str(&self, key: &str) -> Option<Cow<str>>;
}

impl GetI64 for Object {
    fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)
            .and_then(|v| v.as_view().as_scalar())
            .and_then(|s| s.to_integer())
    }
    fn get_str(&self, key: &str) -> Option<Cow<str>> {
        self.get(key)
            .and_then(|v| v.as_view().as_scalar())
            .map(|s| s.into_cow_str())
    }
}

/// Given a cluster, returns a list of all registers inside the cluster,
/// with their names updated to include the cluster index and their address
/// offsets updated to include the cluster address offset.
/// The returned register nodes are as though they were never in a cluster.
fn parse_cluster(
    ctag: &Cluster,
    registers: &mut Vec<Object>,
    cpath: &BlockPath,
    index: &Index,
) -> anyhow::Result<()> {
    let mut cpath = cpath.clone();
    let ctag = if let Some(dfname) = ctag.derived_from.as_ref() {
        let mut ctag = ctag.clone();
        if let Some(path) = derive_cluster(&mut ctag, dfname, &cpath.parent().unwrap(), index)? {
            cpath = path;
        }
        Cow::Owned(ctag)
    } else {
        Cow::Borrowed(ctag)
    };
    match ctag.as_ref() {
        Cluster::Single(c) => {
            let mut regs: Vec<Register> = c.registers().cloned().collect();
            let cluster_addr = c.address_offset;
            for r in &mut regs {
                let rpath = cpath.new_register(&r.name);
                r.name = format!("{} [0]", r.name);
                r.address_offset += cluster_addr;
                parse_register_array(r, registers, &rpath, index)?;
            }
        }
        Cluster::Array(c, d) => {
            for (i, cluster_idx) in d.indexes().enumerate() {
                let mut regs: Vec<Register> = c.registers().cloned().collect();
                let cluster_addr = c.address_offset + (i as u32) * d.dim_increment;
                for r in &mut regs {
                    let rpath = cpath.new_register(&r.name);
                    r.name = format!("{} [{cluster_idx}]", r.name);
                    r.address_offset += cluster_addr;
                    parse_register_array(r, registers, &rpath, index)?;
                }
            }
        }
    }
    Ok(())
}

fn parse_register_array(
    rtag: &Register,
    registers: &mut Vec<Object>,
    rpath: &RegisterPath,
    index: &Index,
) -> anyhow::Result<()> {
    let mut rpath = rpath.clone();
    let rtag = if let Some(dfname) = rtag.derived_from.as_ref() {
        let mut rtag = rtag.clone();
        if let Some(path) = derive_register(&mut rtag, dfname, &rpath.block, index)? {
            rpath = path;
        }
        Cow::Owned(rtag)
    } else {
        Cow::Borrowed(rtag)
    };
    match rtag.as_ref() {
        Register::Single(r) => {
            let register = parse_register(r, &rpath, index)
                .with_context(|| format!("In register {}", r.name))?;
            registers.push(register);
        }
        Register::Array(r, d) => {
            for (i, idx) in d.indexes().enumerate() {
                let mut r = r.clone();
                let idxs = format!("[{idx}]");
                r.name = r.name.replace("[%s]", &idxs).replace("%s", &idxs);
                r.address_offset += (i as u32) * d.dim_increment;
                r.description = r
                    .description
                    .map(|d| d.replace("[%s]", &idx).replace("%s", &idx));
                let register = parse_register(&r, &rpath, index)
                    .with_context(|| format!("In register {}", r.name))?;
                registers.push(register);
            }
        }
    }
    Ok(())
}

fn parse_register(
    rtag: &RegisterInfo,
    rpath: &RegisterPath,
    index: &Index,
) -> anyhow::Result<Object> {
    let mut register_fields_total = 0;
    let mut register_fields_documented = 0;
    let rsize = rtag.properties.size.unwrap_or(32);
    let raccs = rtag
        .properties
        .access
        .map(Access::as_str)
        .unwrap_or("Unspecified");

    let mut flds = Vec::new();
    for f in rtag.fields() {
        let mut fpath = rpath.new_field(&f.name);
        let f = if let Some(dfname) = f.derived_from.as_ref() {
            let mut f = f.clone();
            if let Some(path) = derive_field(&mut f, dfname, rpath, index)? {
                fpath = path;
            }
            f
        } else {
            f.clone()
        };
        match f {
            Field::Single(f) => {
                flds.push((f, fpath));
            }
            Field::Array(f, d) => {
                for (i, idx) in d.indexes().enumerate() {
                    let mut f = f.clone();
                    let idxs = format!("[{idx}]");
                    f.name = f.name.replace("[%s]", &idxs).replace("%s", &idxs);
                    f.bit_range = BitRange::from_offset_width(
                        f.bit_offset() + (i as u32) * d.dim_increment,
                        f.bit_width(),
                    );
                    f.description = f
                        .description
                        .map(|d| d.replace("[%s]", &idx).replace("%s", &idx));
                    flds.push((f, fpath.clone()));
                }
            }
        }
    }

    flds.sort_by_key(|f| f.0.bit_offset());

    let mut filling = 0_u64;

    let mut fields = Vec::with_capacity(flds.len());
    for (ftag, fpath) in &flds {
        register_fields_total += 1;

        let foffset = ftag.bit_offset();
        let fwidth = ftag.bit_width();
        let bit_mask = (u64::MAX >> (u64::BITS - fwidth)) << foffset;
        filling |= bit_mask;

        let faccs = ftag.access.map(Access::as_str).unwrap_or(raccs);
        let enums = ftag.enumerated_values.first();
        let wc = &ftag.write_constraint;
        let mut fdoc = None;
        if enums.is_some() || wc.is_some() || faccs == "read-only" {
            register_fields_documented += 1;
            if let Some(enums) = enums {
                let mut doc = "Allowed values:<br>".to_string();
                let enums = if let Some(dfname) = enums.derived_from.as_ref() {
                    let mut enums = enums.clone();
                    derive_enumerated_values(&mut enums, dfname, fpath, index)?;
                    Cow::Owned(enums)
                } else {
                    Cow::Borrowed(enums)
                };

                for value in &enums.values {
                    let val = if let Some(value) = value.value {
                        value.to_string()
                    } else {
                        let val = value
                            .is_default()
                            .then(|| enums_to_map(&enums))
                            .and_then(|map| minimal_hole(&map, fwidth))
                            .ok_or_else(|| anyhow!("Value is missing from {value:?}"))?;
                        format!("{val} (+)")
                    };

                    doc += &format!(
                        "<strong>{val}: {}</strong>: {}<br>",
                        value.name,
                        sanitize(value.description.as_deref().unwrap_or(""))
                    );
                }
                fdoc = Some(doc);
            } else if let Some(WriteConstraint::Range(wcrange)) = wc.as_ref() {
                let mn = hex(wcrange.min);
                let mx = hex(wcrange.max);
                fdoc = Some(format!("Allowed values: <strong>{mn}-{mx}</strong>"));
            }
        }
        fields.push(object!({
            "name": ftag.name,
            "offset": foffset,
            "width": fwidth,
            "msb": ftag.msb(),
            "description": ftag.description.as_deref().map(sanitize),
            "doc": fdoc,
            "access": faccs,
        }));
    }

    let mut table = vec![
        vec![
            object!({
                "width": 1,
                "doc": false,
                "access": "",
            });
            16
        ];
        2
    ];

    for (ftag, _) in flds.iter().rev() {
        let foffset = ftag.bit_offset();
        let faccs = ftag.access.map(Access::as_str).unwrap_or(raccs);
        let mut access = short_access(faccs).to_string();
        let mwv = ftag
            .modified_write_values
            .unwrap_or(ModifiedWriteValues::Modify);
        let ra = ftag.read_action;
        if access != "N/A" {
            match (faccs, mwv, ra) {
                (_, ModifiedWriteValues::Modify, None) => {}
                ("read-only", _, ra) => access = short_ra(ra).to_string(),
                ("write-only", mwv, _) => access = short_mwv(mwv).to_string(),
                (_, mwv, ra) => access = format!("{}/{}", short_ra(ra), short_mwv(mwv)),
            };
        }
        let fwidth = ftag.bit_width();
        if foffset + fwidth > rsize {
            return Err(anyhow!("Wrong field offset/width"));
        }
        let fdoc = !ftag.enumerated_values.is_empty() || ftag.write_constraint.is_some();
        for idx in foffset..(foffset + fwidth).min(32) {
            let trowidx = ((31 - idx) / 16) as usize;
            let tcolidx = (15 - (idx % 16)) as usize;
            let separated = foffset < 16 && foffset + fwidth > 16;
            let tcell = object!({
                "name": ftag.name,
                "doc": fdoc,
                "access": access,
                "separated": separated,
                "width": table[trowidx][tcolidx].get("width"),
            });
            table[trowidx][tcolidx] = tcell;
        }
    }

    for trow in table.iter_mut() {
        let mut idx = 0;
        while idx < trow.len() - 1 {
            if trow[idx].get("name") == trow[idx + 1].get("name") {
                let mut width = trow[idx].get_i64("width").unwrap();
                width += 1;
                trow[idx].insert("width".into(), Scalar::new(width).into());
                trow.remove(idx + 1);
                continue;
            }
            idx += 1
        }
    }
    let table = vec![
        (filling > u16::MAX as u64)
            .then(|| object!({"headers": (16..32).rev().collect::<Vec<_>>(), "fields": table[0]})),
        (filling > 0)
            .then(|| object!({"headers": (0..16).rev().collect::<Vec<_>>(), "fields": table[1]})),
    ];

    let offset = rtag.address_offset;
    Ok(object!({
        "name": rtag.name,
        "size": rsize,
        "offset_int": offset,
        "offset": hex(offset as _),
        "description": rtag.description.as_deref().map(sanitize),
        "resetValue": format!("0x{:08X}", rtag.properties.reset_value.unwrap_or_default()),
        "access": raccs,
        "writeConstraint": rtag.write_constraint,
        "fields": fields,
        "table": table,
        "fields_total": register_fields_total,
        "fields_documented": register_fields_documented,
        "progress": progress(register_fields_documented, register_fields_total),
    }))
}

fn parse_device(svdfile: impl AsRef<Path>) -> anyhow::Result<Object> {
    let svdfile = svdfile.as_ref();
    let mut file = File::open(svdfile)?;
    #[cfg(not(target_os = "windows"))]
    let temp = file.metadata()?.st_mtime();
    #[cfg(target_os = "windows")]
    let temp = file.metadata()?.last_write_time() as i64;
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;
    let device = svd_parser::parse_with_config(
        &xml,
        &svd_parser::Config::default().expand_properties(true),
    )?;
    let index = Index::create(&device);
    let mut peripherals = Vec::new();
    let mut device_fields_total = 0;
    let mut device_fields_documented = 0;
    let mut ptags = device.peripherals.iter().collect::<Vec<_>>();
    ptags.sort_by_key(|p| p.name.to_lowercase());
    for ptag in ptags {
        let mut registers = Vec::new();
        let mut peripheral_fields_total = 0;
        let mut peripheral_fields_documented = 0;
        let pname = &ptag.name;
        let mut ppath = BlockPath::new(&ptag.name);
        let ptag = if let Some(dfname) = ptag.derived_from.as_ref() {
            let mut ptag = ptag.clone();
            if let Some(path) = derive_peripheral(&mut ptag, dfname, &index)? {
                ppath = path;
            }
            Cow::Owned(ptag)
        } else {
            Cow::Borrowed(ptag)
        };
        for ctag in ptag.clusters() {
            let cpath = ppath.new_cluster(&ctag.name);
            parse_cluster(ctag, &mut registers, &cpath, &index)
                .with_context(|| format!("In cluster {}", ctag.name))
                .with_context(|| format!("In peripheral {}", ptag.name))?;
        }
        for rtag in ptag.registers() {
            let rpath = ppath.new_register(&rtag.name);
            parse_register_array(rtag, &mut registers, &rpath, &index)
                .with_context(|| format!("In peripheral {}", ptag.name))?;
        }

        registers.sort_by_key(|r| {
            (
                r.get_i64("offset_int"),
                r.get_str("name").map(|s| s.to_lowercase()),
            )
        });

        for register in &registers {
            peripheral_fields_total += register.get_i64("fields_total").unwrap();
            peripheral_fields_documented += register.get_i64("fields_documented").unwrap();
        }

        peripherals.push(object!({
            "name": pname,
            "base": format!("0x{:08x}", ptag.base_address),
            "description": ptag.description.as_deref().map(sanitize),
            "registers": registers,
            "fields_total": peripheral_fields_total,
            "fields_documented": peripheral_fields_documented,
            "progress": progress(peripheral_fields_documented, peripheral_fields_total),
        }));
        device_fields_total += peripheral_fields_total;
        device_fields_documented += peripheral_fields_documented;
    }

    Ok(object!({
        "name": device.name,
        "peripherals": peripherals,
        "fields_total": device_fields_total,
        "fields_documented": device_fields_documented,
        "last-modified": temp,
        "svdfile": svdfile.to_str().unwrap(),
        "progress": progress(device_fields_documented, device_fields_total),
    }))
}

fn process_svd(svdfile: impl AsRef<Path>) -> anyhow::Result<Object> {
    let svdfile = svdfile.as_ref().to_str().unwrap();
    println!("Processing {}", svdfile);
    parse_device(svdfile).with_context(|| format!("In file {svdfile}"))
}

fn generate_if_newer(
    template: &liquid::Template,
    device: &Object,
    htmldir: &Path,
) -> anyhow::Result<()> {
    let pagename = format!("{}.html", device.get_str("name").unwrap());
    let filename = htmldir.join(&pagename);

    #[cfg(not(target_os = "windows"))]
    let file_mtime = if filename.is_file() {
        std::fs::metadata(&filename)?.st_mtime()
    } else {
        i64::MIN
    };

    #[cfg(target_os = "windows")]
    let file_mtime = if filename.is_file() {
        std::fs::metadata(&filename)?.last_write_time() as i64
    } else {
        i64::MIN
    };

    if !filename.is_file() || file_mtime < device.get_i64("last-modified").unwrap() {
        println!("Generating {pagename}");
        let svdfile = device.get_str("svdfile").unwrap();
        let svdfile = Path::new(svdfile.as_ref());
        let svdfile_name = svdfile.file_name().unwrap();
        let mut file = std::fs::File::create(filename)?;
        generate_device_page(template, device, &mut file)?;
        std::fs::copy(svdfile, htmldir.join(svdfile_name))?;
    }

    Ok(())
}

pub fn svd2html(htmldir: &Path, svdfiles: &[PathBuf]) -> anyhow::Result<()> {
    let svdfiles = svdfiles.iter().filter(|&f| f.is_file()).collect::<Vec<_>>();

    if !htmldir.exists() {
        std::fs::create_dir(htmldir)?;
    }
    let template_file = include_str!("template.html");
    let template = liquid::ParserBuilder::with_stdlib()
        .build()
        .unwrap()
        .parse(template_file)
        .unwrap();
    let mut devices = svdfiles
        .par_iter()
        .map(|f| {
            let device = process_svd(f).unwrap();
            generate_if_newer(&template, &device, htmldir).unwrap();
            object!({
                "name": device.get("name"),
                "progress": device.get("progress"),
                "fields_documented": device.get("fields_documented"),
                "fields_total": device.get("fields_total"),
            })
        })
        .collect::<Vec<_>>();
    devices.sort_by_key(|d| d.get_str("name").map(|s| s.to_lowercase()));

    let mut file = std::fs::File::create(htmldir.join("index.html"))?;
    generate_index_page(&devices, &mut file)?;
    Ok(())
}

fn enums_to_map(evs: &EnumeratedValues) -> BTreeMap<u64, &EnumeratedValue> {
    let mut map = BTreeMap::new();
    for ev in &evs.values {
        if let Some(v) = ev.value {
            map.insert(v, ev);
        }
    }
    map
}

fn minimal_hole(map: &BTreeMap<u64, &EnumeratedValue>, width: u32) -> Option<u64> {
    (0..(1u64 << width)).find(|&v| !map.contains_key(&v))
}
