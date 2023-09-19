use anyhow::Result;
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use svd_parser::Config;
use svd_rs::Device;

struct Part {
    pub name: String,
    pub device: Device,
}

fn parse(in_path: &Path) -> Result<Part> {
    let mut input = String::new();
    File::open(in_path)?.read_to_string(&mut input)?;

    let device = svd_parser::parse_with_config(&input, &Config::default().expand(true))?;
    let name = in_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .split('.')
        .next()
        .unwrap()
        .to_string();
    //dbg!(&name);
    Ok(Part { name, device })
}

fn html_page(title: &str, table: &str) -> String {
    let title = format!("<title>{title}</title>");
    let header = format!("<h1>{title}</h1>");
    let out = [
        r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">"#,
        &title,
        r##"</head>
<style>
table thead tr th {
    position: sticky;
    top: 0;
    z-index: 6;
    background: white;
}
td:first-child {
    position: sticky;
    left: 0;
    z-index: 5;
    background: white;
}
</style>
<body>"##,
        &header,
        table,
        "</body></html>",
    ];
    out.join("\n")
}

fn who_has_what_peripherals(parts: &[Part]) -> BTreeMap<(String, u64), Vec<String>> {
    let mut peripherals: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    for part in parts {
        for periph in &part.device.peripherals {
            let name = (periph.name.clone(), periph.base_address);
            peripherals.entry(name).or_default().push(part.name.clone());
        }
    }
    peripherals
}

fn html_table_peripherals(
    parts: &[Part],
    peripherals: &BTreeMap<(String, u64), Vec<String>>,
) -> String {
    let mut out = "<table><thead><tr><th>Peripheral</th><th>Address</th>\n".to_string();
    for part in parts {
        out.push_str(&format!("<th>{}</th>\n", part.device.name));
    }
    out.push_str("</thead><tbody>\n");
    for ((name, base), periph_parts) in peripherals {
        let base = format!("0x{base:08X}");
        let link = format!(r#"<a href="{name}_{base}.html">{name}</a>"#);
        out.push_str(&format!("<tr><td>{link}</td><td>{base}</td>\n"));
        for part in parts {
            if periph_parts.contains(&part.name) {
                out.push_str(r##"<td align=center bgcolor="#ccffcc">&#10004;</td>"##);
            } else {
                out.push_str(r##"<td align=center bgcolor="#ffcccc">&#10008;</td>"##);
            }
            out.push('\n');
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tr>\n");
    out.push_str("</tbody></table>\n");
    out
}

fn who_has_what_peripheral_registers(
    parts: &[Part],
    peripheral: &(String, u64),
) -> BTreeMap<(u32, String), Vec<String>> {
    let mut registers: BTreeMap<(u32, String), Vec<String>> = BTreeMap::new();
    for part in parts {
        for periph in &part.device.peripherals {
            if periph.name != peripheral.0 || periph.base_address != peripheral.1 {
                continue;
            }
            for reg in periph.all_registers() {
                let name = (reg.address_offset, reg.name.clone());
                registers.entry(name).or_default().push(part.name.clone());
            }
        }
    }
    registers
}

fn html_table_registers(
    parts: &[Part],
    peripheral: &(String, u64),
    registers: &BTreeMap<(u32, String), Vec<String>>,
) -> String {
    let mut out = "<table><thead><tr><th>Register</th><th>Offset</th>\n".to_string();
    for part in parts {
        out.push_str(&format!("<th>{}</th>\n", part.device.name));
    }
    out.push_str("</thead><tbody>\n");
    for ((offset, name), reg_parts) in registers {
        let offset = format!("0x{offset:04X}");
        let link = format!(
            r#"<a href="{}_0x{:08X}_{name}_{offset}.html">{name}</a>"#,
            peripheral.0, peripheral.1
        );
        out.push_str(&format!("<tr><td>{link}</td><td>{offset}</td>\n"));
        for part in parts {
            if reg_parts.contains(&part.name) {
                out.push_str(r##"<td align=center bgcolor="#ccffcc">&#10004;</td>"##);
            } else {
                out.push_str(r##"<td align=center bgcolor="#ffcccc">&#10008;</td>"##);
            }
            out.push('\n');
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tr>\n");
    out.push_str("</tbody></table>");
    out
}

fn who_has_what_register_fields(
    parts: &[Part],
    peripheral: &(String, u64),
    register: &(u32, String),
) -> BTreeMap<(u32, u32, String), Vec<String>> {
    let mut fields: BTreeMap<(u32, u32, String), Vec<String>> = BTreeMap::new();
    for part in parts {
        for periph in &part.device.peripherals {
            if periph.name != peripheral.0 || periph.base_address != peripheral.1 {
                continue;
            }
            for reg in periph.all_registers() {
                if reg.name != register.1 || reg.address_offset != register.0 {
                    continue;
                }
                for field in reg.fields() {
                    let name = (field.bit_offset(), field.bit_width(), field.name.clone());
                    fields.entry(name).or_default().push(part.name.clone());
                }
            }
        }
    }
    fields
}

fn html_table_fields(parts: &[Part], fields: &BTreeMap<(u32, u32, String), Vec<String>>) -> String {
    let mut out = "<table><thead><tr><th>Field</th><th>Offset</th><th>Width</th>\n".to_string();
    for part in parts {
        out.push_str(&format!("<th>{}</th>\n", part.device.name));
    }
    out.push_str("</thead><tbody>\n");
    for ((offset, width, name), field_parts) in fields {
        out.push_str(&format!(
            "<tr><td>{name}</td><td>{offset}</td><td>{width}</td>\n"
        ));
        for part in parts {
            if field_parts.contains(&part.name) {
                out.push_str(r##"<td align=center bgcolor="#ccffcc">&#10004;</td>"##);
            } else {
                out.push_str(r##"<td align=center bgcolor="#ffcccc">&#10008;</td>"##);
            }
            out.push('\n');
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tr>\n");
    out.push_str("</tbody></table>");
    out
}

fn html_tables(parts: &[Part]) -> HashMap<String, String> {
    let peripherals = who_has_what_peripherals(parts);
    let mut files = HashMap::new();
    let peripheral_table = html_table_peripherals(parts, &peripherals);
    let peripheral_title = "Compare peripherals";
    files.insert(
        "index.html".to_string(),
        html_page(peripheral_title, &peripheral_table),
    );
    for pname in peripherals.keys() {
        let registers = who_has_what_peripheral_registers(parts, pname);
        let register_table = html_table_registers(parts, pname, &registers);
        let register_title = format!("Registers In {} 0x{:08X}", pname.0, pname.1);
        let mut filename = format!("{}_0x{:08X}.html", pname.0, pname.1);
        files.insert(filename, html_page(&register_title, &register_table));
        for rname in registers.keys() {
            let fields = who_has_what_register_fields(parts, pname, rname);
            let field_table = html_table_fields(parts, &fields);
            let field_title = format!(
                "Fields In {}_{} (0x{:08X}, 0x{:04X})",
                pname.0, rname.1, pname.1, rname.0
            );
            filename = format!(
                "{}_0x{:08X}_{}_0x{:04X}.html",
                pname.0, pname.1, rname.1, rname.0
            );
            files.insert(filename, html_page(&field_title, &field_table));
        }
    }
    files
}

pub fn htmlcompare(htmldir: &Path, svdfiles: &[PathBuf]) -> Result<()> {
    let parts = svdfiles
        .iter()
        .map(|p| parse(p))
        .collect::<Result<Vec<_>>>()?;
    let files = html_tables(&parts);
    std::fs::create_dir_all(htmldir)?;
    for file in files {
        let f = htmldir.join(file.0);
        let mut f = File::create(f)?;
        f.write_all(file.1.as_bytes())?;
    }
    Ok(())
}
