use svd_parser::svd::{
    BitRange, DimElement, EnumeratedValues, Field, FieldInfo, Register, RegisterInfo, Usage,
    WriteConstraint, WriteConstraintRange,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIterMut, Matched, OptIter};
use super::yaml_ext::{parse_i64, GetVal, ToYaml};
use super::{check_offsets, matchname, spec_ind, VAL_LVL};
use super::{make_derived_enumerated_values, make_ev_array, make_ev_name, make_field};

pub type FieldIterMut<'a> = OptIter<&'a mut Field, std::slice::IterMut<'a, Field>>;
pub type FieldMatchIterMut<'a, 'b> = MatchIterMut<'a, 'b, Field, FieldIterMut<'a>>;

pub trait RegisterInfoExt {
    /// Calculate filling of register
    fn get_bitmask(&self) -> u64;
}

impl RegisterInfoExt for RegisterInfo {
    fn get_bitmask(&self) -> u64 {
        let mut mask = 0x0;
        if let Some(fields) = self.fields.as_ref() {
            for ftag in fields {
                mask |= (!0 >> (64 - ftag.bit_range.width)) << ftag.bit_range.offset;
            }
        }
        mask
    }
}

/// Collecting methods for processing register contents
pub trait RegisterExt {
    /// Work through a register, handling all fields
    fn process(&mut self, rmod: &Hash, pname: &str, update_fields: bool);

    /// Add fname given by fadd to rtag
    fn add_field(&mut self, fname: &str, fadd: &Hash);

    /// Delete fields matched by fspec inside rtag
    fn delete_field(&mut self, fspec: &str);

    /// Clear contents of fields matched by fspec inside rtag
    fn clear_field(&mut self, fspec: &str);

    /// Iterates over all fields
    fn iter_all_fields<'a>(&'a mut self) -> FieldIterMut<'a>;

    /// Iterates over all fields that match fspec and live inside rtag
    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b>;

    /// Work through a field, handling either an enum or a range
    fn process_field(&mut self, pname: &str, fspec: &str, fmod: &Yaml);

    /// Add an enumeratedValues given by field to all fspec in rtag
    fn process_field_enum(&mut self, pname: &str, fspec: &str, fmod: &Hash, usage: Usage);

    /// Add a writeConstraint range given by field to all fspec in rtag
    fn process_field_range(&mut self, pname: &str, fspec: &str, fmod: &Vec<Yaml>);

    /// Delete substring from the beginning bitfield names inside rtag
    fn strip_start(&mut self, substr: &str);

    /// Delete substring from the ending bitfield names inside rtag
    fn strip_end(&mut self, substr: &str);

    /// Modify fspec inside rtag according to fmod
    fn modify_field(&mut self, fspec: &str, fmod: &Hash);

    /// Merge all fspec in rtag.
    /// Support list of field to auto-merge, and dict with fspec or list of fspec
    fn merge_fields(&mut self, key: &str, value: Option<&Yaml>);

    /// Split all fspec in rtag.
    /// Name and description can be customized with %s as a placeholder to the iterator value
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash);

    /// Collect same fields in peripheral into register array
    fn collect_fields_in_array(&mut self, fspec: &str, fmod: &Hash);
}

impl RegisterExt for Register {
    fn process(&mut self, rmod: &Hash, pname: &str, update_fields: bool) {
        // Handle deletions
        for fspec in rmod.str_vec_iter("_delete") {
            self.delete_field(fspec);
        }

        // Handle field clearing
        for fspec in rmod.str_vec_iter("_clear") {
            self.clear_field(fspec);
        }

        // Handle modifications
        for (fspec, fmod) in rmod.hash_iter("_modify") {
            self.modify_field(fspec.as_str().unwrap(), fmod.as_hash().unwrap())
        }
        // Handle additions
        for (fname, fadd) in rmod.hash_iter("_add") {
            self.add_field(fname.as_str().unwrap(), fadd.as_hash().unwrap());
        }

        // Handle merges
        match rmod.get(&"_merge".to_yaml()) {
            Some(Yaml::Hash(h)) => {
                for (fspec, fmerge) in h {
                    self.merge_fields(fspec.as_str().unwrap(), Some(fmerge));
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    self.merge_fields(fspec.as_str().unwrap(), None);
                }
            }
            _ => {}
        }

        // Handle splits
        match rmod.get(&"_split".to_yaml()) {
            Some(Yaml::Hash(h)) => {
                for (fspec, fsplit) in h {
                    self.split_fields(fspec.as_str().unwrap(), fsplit.as_hash().unwrap());
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    self.split_fields(fspec.as_str().unwrap(), &Hash::new());
                }
            }
            _ => {}
        }

        // Handle strips
        for prefix in rmod.str_vec_iter("_strip") {
            self.strip_start(prefix);
        }
        for suffix in rmod.str_vec_iter("_strip_end") {
            self.strip_end(suffix);
        }

        // Handle fields
        if update_fields {
            for (fspec, field) in rmod {
                let fspec = fspec.as_str().unwrap();
                if !fspec.starts_with("_") {
                    self.process_field(pname, fspec, field)
                }
            }
        }

        // Handle field arrays
        for (fspec, fmod) in rmod.hash_iter("_array") {
            self.collect_fields_in_array(fspec.as_str().unwrap(), fmod.as_hash().unwrap())
        }
    }

    fn iter_all_fields<'a>(&'a mut self) -> FieldIterMut<'a> {
        FieldIterMut::new(self.fields.as_mut().map(|f| f.iter_mut()))
    }

    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b> {
        self.iter_all_fields().matched(spec)
    }

    fn strip_start(&mut self, substr: &str) {
        let len = substr.len();
        let glob = globset::Glob::new(&(substr.to_string() + "*"))
            .unwrap()
            .compile_matcher();
        for ftag in self.iter_all_fields() {
            if glob.is_match(&ftag.name) {
                ftag.name.drain(..len);
            }
        }
    }

    fn strip_end(&mut self, substr: &str) {
        let len = substr.len();
        let glob = globset::Glob::new(&("*".to_string() + substr))
            .unwrap()
            .compile_matcher();
        for ftag in self.iter_all_fields() {
            if glob.is_match(&ftag.name) {
                let nlen = ftag.name.len();
                ftag.name.truncate(nlen - len);
            }
        }
    }

    fn modify_field(&mut self, fspec: &str, fmod: &Hash) {
        for ftag in self.iter_fields(fspec) {
            if let Some(wc) = fmod
                .get(&"_write_constraint".to_yaml())
                .or_else(|| fmod.get(&"writeConstraint".to_yaml()))
                .map(|value| match value {
                    Yaml::String(s) if s == "none" => {
                        // Completely remove the existing writeConstraint
                        None
                    }
                    Yaml::String(s) if s == "enum" => {
                        // Only allow enumerated values
                        Some(WriteConstraint::UseEnumeratedValues(true))
                    }
                    Yaml::Array(a) => {
                        // Allow a certain range
                        Some(WriteConstraint::Range(WriteConstraintRange {
                            min: parse_i64(&a[0]).unwrap() as u64,
                            max: parse_i64(&a[1]).unwrap() as u64,
                        }))
                    }
                    _ => panic!("Unknown writeConstraint type {:?}", value),
                })
            {
                ftag.write_constraint = wc;
            }
            // For all other tags, just set the value
            ftag.modify_from(make_field(fmod), VAL_LVL).unwrap();
        }
    }

    fn add_field(&mut self, fname: &str, fadd: &Hash) {
        if self.iter_all_fields().find(|f| f.name == fname).is_some() {
            panic!("register {} already has a field {}", self.name, fname);
        }
        // TODO: add field arrays
        let fnew = Field::Single(make_field(fadd).name(fname.into()).build(VAL_LVL).unwrap());
        self.fields.get_or_insert_with(Default::default).push(fnew);
    }

    fn delete_field(&mut self, fspec: &str) {
        if let Some(fields) = self.fields.as_mut() {
            fields.retain(|f| !(matchname(&f.name, fspec)));
        }
    }

    fn clear_field(&mut self, fspec: &str) {
        for ftag in self.iter_fields(fspec) {
            ftag.enumerated_values = Vec::new();
            ftag.write_constraint = None;
        }
    }

    fn merge_fields(&mut self, key: &str, value: Option<&Yaml>) {
        let (name, names) = match value {
            Some(Yaml::String(value)) => (
                key.to_string(),
                self.iter_fields(&value)
                    .map(|f| f.name.to_string())
                    .collect(),
            ),
            Some(Yaml::Array(value)) => {
                let mut names = Vec::new();
                for fspec in value {
                    names.extend(
                        self.iter_fields(fspec.as_str().unwrap())
                            .map(|f| f.name.to_string()),
                    );
                }
                (key.to_string(), names)
            }
            Some(_) => panic!("Invalid usage of merge for {}.{}", self.name, key),
            None => {
                let names: Vec<String> =
                    self.iter_fields(key).map(|f| f.name.to_string()).collect();
                let name = commands::util::longest_common_prefix(
                    names.iter().map(|n| n.as_str()).collect(),
                )
                .to_string();
                (name, names)
            }
        };

        if names.is_empty() {
            panic!("Could not find any fields to merge {}.{}", self.name, key);
        }
        let mut bitwidth = 0;
        let mut bitoffset = u32::MAX;
        let mut first = true;
        let mut desc = None;
        if let Some(fields) = self.fields.as_mut() {
            for f in fields.iter_mut() {
                if names.contains(&f.name) {
                    if first {
                        desc = f.description.clone();
                        first = false;
                    }
                    bitwidth += f.bit_range.width;
                    bitoffset = bitoffset.min(f.bit_range.offset);
                }
            }
            fields.retain(|f| !names.contains(&f.name));
            fields.push(Field::Single(
                FieldInfo::builder()
                    .name(name)
                    .description(desc)
                    .bit_range(BitRange::from_offset_width(bitoffset, bitwidth))
                    .build(VAL_LVL)
                    .unwrap(),
            ));
        }
    }

    fn collect_fields_in_array(&mut self, fspec: &str, fmod: &Hash) {
        if let Some(fs) = self.fields.as_mut() {
            let mut fields = Vec::new();
            let mut place = usize::MAX;
            let mut i = 0;
            let (li, ri) = spec_ind(fspec);
            while i < fs.len() {
                match &fs[i] {
                    Field::Single(f) if matchname(&f.name, fspec) => {
                        if let Field::Single(f) = fs.remove(i) {
                            fields.push(f);
                            place = place.min(i);
                        }
                    }
                    _ => i += 1,
                }
            }
            if fields.is_empty() {
                panic!("{}: fields {} not found", self.name, fspec);
            }
            fields.sort_by(|f1, f2| f1.bit_range.offset.cmp(&f2.bit_range.offset));
            let dim = fields.len();
            let dim_index = if fmod.contains_key(&"_start_from_zero".to_yaml()) {
                (0..dim).map(|v| v.to_string()).collect::<Vec<_>>()
            } else {
                fields
                    .iter()
                    .map(|f| f.name[li..f.name.len() - ri].to_string())
                    .collect::<Vec<_>>()
            };
            let offsets = fields
                .iter()
                .map(|f| f.bit_range.offset)
                .collect::<Vec<_>>();
            let dim_increment = if dim > 1 { offsets[1] - offsets[0] } else { 0 };
            if !check_offsets(&offsets, dim_increment) {
                panic!(
                    "{}: registers cannot be collected into {} array",
                    self.name, fspec
                );
            }
            let mut finfo = fields.swap_remove(0);
            if let Some(name) = fmod.get_str("name") {
                finfo.name = name.into();
            } else {
                finfo.name = format!("{}%s{}", &fspec[..li], &fspec[fspec.len() - ri..]);
            }
            if dim_index[0] == "0" {
                if let Some(desc) = finfo.description.as_mut() {
                    *desc = desc.replace('0', "%s");
                }
            }
            let field = Field::Array(
                finfo,
                DimElement::builder()
                    .dim(dim as u32)
                    .dim_increment(dim_increment)
                    .dim_index(Some(dim_index))
                    .build(VAL_LVL)
                    .unwrap(),
            );
            //field.process(fmod, &self.name, true);
            fs.insert(place, field);
        }
    }
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash) {
        let mut it = self.iter_fields(fspec);
        let (new_fields, name) = match (it.next(), it.next()) {
            (None, _) => panic!("Could not find any fields to split {}.{}", self.name, fspec),
            (Some(_), Some(_)) => panic!(
                "Only one field can be spitted at time {}.{}",
                self.name, fspec
            ),
            (Some(first), None) => {
                let name = if let Some(n) = fsplit.get_str("name") {
                    n.to_string()
                } else {
                    first.name.clone() + "%s"
                };
                let desc = if let Some(d) = fsplit.get_str("description") {
                    Some(d.to_string())
                } else {
                    first.description.clone()
                };
                let bitoffset = first.bit_range.offset;
                (
                    (0..first.bit_range.width)
                        .map(|i| {
                            let is = i.to_string();
                            Field::Single(
                                FieldInfo::builder()
                                    .name(name.replace("%s", &is))
                                    .description(desc.clone().map(|d| d.replace("%s", &is)))
                                    .bit_range(BitRange::from_offset_width(bitoffset + i, 1))
                                    .build(VAL_LVL)
                                    .unwrap(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    first.name.to_string(),
                )
            }
        };
        if let Some(fields) = self.fields.as_mut() {
            fields.retain(|f| f.name != name);
            fields.extend(new_fields);
        }
    }

    fn process_field(&mut self, pname: &str, fspec: &str, fmod: &Yaml) {
        match fmod {
            Yaml::Hash(fmod) => {
                let is_read = fmod.contains_key(&"_read".to_yaml());
                let is_write = fmod.contains_key(&"_write".to_yaml());
                if !(is_read || is_write) {
                    self.process_field_enum(pname, fspec, fmod, Usage::ReadWrite);
                } else {
                    if is_read {
                        self.process_field_enum(
                            pname,
                            fspec,
                            fmod.get_hash("_read").unwrap(),
                            Usage::Read,
                        );
                    }
                    if is_write {
                        self.process_field_enum(
                            pname,
                            fspec,
                            fmod.get_hash("_write").unwrap(),
                            Usage::Write,
                        );
                    }
                }
            }
            Yaml::Array(fmod) if fmod.len() == 2 => {
                self.process_field_range(pname, fspec, fmod);
            }
            _ => {}
        }
    }

    fn process_field_enum(&mut self, pname: &str, fspec: &str, mut fmod: &Hash, usage: Usage) {
        fn set_enum(f: &mut FieldInfo, val: EnumeratedValues, usage: Usage, replace: bool) {
            if usage == Usage::ReadWrite {
                if f.enumerated_values.is_empty() || replace {
                    f.enumerated_values = vec![val];
                } else {
                    panic!("field {} already has {:?} enumeratedValues", f.name, usage);
                }
            } else {
                match f.enumerated_values.as_mut_slice() {
                    [] => f.enumerated_values.push(val),
                    [v] => {
                        if v.usage == Some(usage) || v.usage == Some(Usage::ReadWrite) {
                            if replace {
                                *v = val.clone();
                            } else {
                                panic!("field {} already has {:?} enumeratedValues", f.name, usage);
                            }
                        } else {
                            f.enumerated_values.push(val.clone());
                        }
                    }
                    [v1, v2] => {
                        if replace {
                            if v1.usage == Some(usage) {
                                *v1 = val.clone();
                            }
                            if v2.usage == Some(usage) {
                                *v2 = val.clone();
                            }
                        } else {
                            panic!("field {} already has {:?} enumeratedValues", f.name, usage);
                        }
                    }
                    _ => panic!("Incorrect enumeratedValues"),
                }
            }
        }

        let mut replace_if_exists = false;
        if let Some(h) = fmod.get_hash("_replace_enum") {
            fmod = h;
            replace_if_exists = true;
        }

        if let Some(d) = fmod.get_str("_derivedFrom") {
            // This is a derived enumeratedValues => Try to find the
            // original definition to extract its <usage>
            let mut derived_enums = self
                .fields
                .as_ref()
                .unwrap()
                .iter()
                .flat_map(|f| f.enumerated_values.iter())
                .filter(|e| e.name.as_deref() == Some(d));
            let orig_usage = match (derived_enums.next(), derived_enums.next()) {
                (Some(e), None) => e.usage(),
                (None, _) => panic!("{}: enumeratedValues {} can't be found", pname, d),
                (Some(_), Some(_)) => {
                    panic!("{}: enumeratedValues {} was found multiple times", pname, d)
                }
            };
            assert_eq!(usage, orig_usage);
            let evs = make_derived_enumerated_values(d);
            for ftag in self.iter_fields(fspec) {
                assert!(
                    ftag.name != d,
                    "EnumeratedValues can't be derived from itself"
                );
                set_enum(ftag, evs.clone(), usage, true);
            }
        } else {
            let offsets = self
                .iter_fields(fspec)
                .map(|f| (f.bit_range.offset, f.name.to_string()))
                .collect::<Vec<_>>();
            if offsets.is_empty() {
                panic!("Could not find {}:{}.{}", pname, &self.name, fspec);
            }
            let (min_offset, name) = offsets.iter().min_by(|on1, on2| on1.0.cmp(&on2.0)).unwrap();
            let name = make_ev_name(name, usage);
            for ftag in self.iter_fields(fspec) {
                if ftag.bit_range.offset == *min_offset {
                    let evs = make_ev_array(fmod)
                        .name(Some(name.clone()))
                        .usage(Some(usage))
                        .build(VAL_LVL)
                        .unwrap();
                    set_enum(ftag, evs, usage, replace_if_exists);
                } else {
                    set_enum(ftag, make_derived_enumerated_values(&name), usage, true);
                }
            }
        }
    }

    fn process_field_range(&mut self, pname: &str, fspec: &str, fmod: &Vec<Yaml>) {
        let mut set_any = false;
        for ftag in self.iter_fields(fspec) {
            ftag.write_constraint = Some(WriteConstraint::Range(WriteConstraintRange {
                min: parse_i64(&fmod[0]).unwrap() as u64,
                max: parse_i64(&fmod[1]).unwrap() as u64,
            }));
            set_any = true;
        }
        if !set_any {
            panic!("Could not find {}:{}.{}", pname, &self.name, fspec);
        }
    }
}
