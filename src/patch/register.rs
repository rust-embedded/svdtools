use anyhow::{anyhow, Context};
use svd_parser::svd::{
    Access, BitRange, DimElement, EnumeratedValues, Field, FieldInfo, ModifiedWriteValues,
    ReadAction, Register, RegisterInfo, Usage, ValidateLevel, WriteConstraint,
    WriteConstraintRange,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIter, Matched};
use super::yaml_ext::{AsType, GetVal, ToYaml};
use super::{
    check_offsets, make_dim_element, matchname, modify_dim_element, spec_ind, PatchResult, VAL_LVL,
};
use super::{make_derived_enumerated_values, make_ev_array, make_ev_name, make_field};

pub type FieldMatchIterMut<'a, 'b> = MatchIter<'b, std::slice::IterMut<'a, Field>>;

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
    fn process(&mut self, rmod: &Hash, pname: &str, update_fields: bool) -> PatchResult;

    /// Add fname given by fadd to rtag
    fn add_field(&mut self, fname: &str, fadd: &Hash) -> PatchResult;

    /// Delete fields matched by fspec inside rtag
    fn delete_field(&mut self, fspec: &str) -> PatchResult;

    /// Clear contents of fields matched by fspec inside rtag
    fn clear_field(&mut self, fspec: &str) -> PatchResult;

    /// Iterates over all fields that match fspec and live inside rtag
    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b>;

    /// Work through a field, handling either an enum or a range
    fn process_field(&mut self, pname: &str, fspec: &str, fmod: &Yaml) -> PatchResult;

    /// Add an enumeratedValues given by field to all fspec in rtag
    fn process_field_enum(
        &mut self,
        pname: &str,
        fspec: &str,
        fmod: &Hash,
        usage: Option<Usage>,
    ) -> PatchResult;

    /// Set readAction for field
    fn set_field_read_action(&mut self, fspec: &str, action: ReadAction);

    /// Set modifiedWriteValues for field
    fn set_field_modified_write_values(&mut self, fspec: &str, mwv: ModifiedWriteValues);

    /// Add a writeConstraint range given by field to all fspec in rtag
    fn process_field_range(&mut self, pname: &str, fspec: &str, fmod: &[Yaml]) -> PatchResult;

    /// Delete substring from the beginning bitfield names inside rtag
    fn strip_start(&mut self, substr: &str) -> PatchResult;

    /// Delete substring from the ending bitfield names inside rtag
    fn strip_end(&mut self, substr: &str) -> PatchResult;

    /// Modify fspec inside rtag according to fmod
    fn modify_field(&mut self, fspec: &str, fmod: &Hash) -> PatchResult;

    /// Merge all fspec in rtag.
    /// Support list of field to auto-merge, and dict with fspec or list of fspec
    fn merge_fields(&mut self, key: &str, value: Option<&Yaml>) -> PatchResult;

    /// Split all fspec in rtag.
    /// Name and description can be customized with %s as a placeholder to the iterator value
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash) -> PatchResult;

    /// Collect same fields in peripheral into register array
    fn collect_fields_in_array(&mut self, fspec: &str, fmod: &Hash) -> PatchResult;
}

impl RegisterExt for Register {
    fn process(&mut self, rmod: &Hash, pname: &str, update_fields: bool) -> PatchResult {
        if self.derived_from.is_some() {
            return Ok(());
        }
        // Handle deletions
        for fspec in rmod.str_vec_iter("_delete")? {
            self.delete_field(fspec)
                .with_context(|| format!("Deleting fields matched to `{fspec}`"))?;
        }

        // Handle strips
        for prefix in rmod.str_vec_iter("_strip")? {
            self.strip_start(prefix)
                .with_context(|| format!("Stripping prefix `{prefix}` from field names"))?;
        }
        for suffix in rmod.str_vec_iter("_strip_end")? {
            self.strip_end(suffix)
                .with_context(|| format!("Stripping suffix `{suffix}` from field names"))?;
        }

        // Handle field clearing
        for fspec in rmod.str_vec_iter("_clear")? {
            self.clear_field(fspec)
                .with_context(|| format!("Clearing contents of fields matched to `{fspec}`"))?;
        }

        // Handle modifications
        for (fspec, fmod) in rmod.hash_iter("_modify") {
            let fspec = fspec.str()?;
            self.modify_field(fspec, fmod.hash()?)
                .with_context(|| format!("Modifying fields matched to `{fspec}`"))?;
        }
        // Handle additions
        for (fname, fadd) in rmod.hash_iter("_add") {
            let fname = fname.str()?;
            self.add_field(fname, fadd.hash()?)
                .with_context(|| format!("Adding field `{fname}`"))?;
        }

        // Handle merges
        match rmod.get(&"_merge".to_yaml()) {
            Some(Yaml::Hash(h)) => {
                for (fspec, fmerge) in h {
                    let fspec = fspec.str()?;
                    self.merge_fields(fspec, Some(fmerge))
                        .with_context(|| format!("Merging fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    let fspec = fspec.str()?;
                    self.merge_fields(fspec, None)
                        .with_context(|| format!("Merging fields matched to `{fspec}`"))?;
                }
            }
            _ => {}
        }

        // Handle splits
        match rmod.get(&"_split".to_yaml()) {
            Some(Yaml::Hash(h)) => {
                for (fspec, fsplit) in h {
                    let fspec = fspec.str()?;
                    self.split_fields(fspec, fsplit.hash()?)
                        .with_context(|| format!("Splitting fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    let fspec = fspec.str()?;
                    self.split_fields(fspec, &Hash::new())
                        .with_context(|| format!("Splitting fields matched to `{fspec}`"))?;
                }
            }
            _ => {}
        }

        // Handle fields
        if update_fields {
            for (fspec, field) in rmod {
                let fspec = fspec.str()?;
                if !fspec.starts_with('_') {
                    self.process_field(pname, fspec, field)
                        .with_context(|| format!("Processing field matched to `{fspec}`"))?;
                }
            }
        }

        // Handle field arrays
        for (fspec, fmod) in rmod.hash_iter("_array") {
            let fspec = fspec.str()?;
            self.collect_fields_in_array(fspec, fmod.hash()?)
                .with_context(|| format!("Collecting fields matched to `{fspec}` in array"))?;
        }

        Ok(())
    }

    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b> {
        self.fields_mut().matched(spec)
    }

    fn strip_start(&mut self, substr: &str) -> PatchResult {
        let len = substr.len();
        let glob = globset::Glob::new(&(substr.to_string() + "*"))?.compile_matcher();
        for ftag in self.fields_mut() {
            if glob.is_match(&ftag.name) {
                ftag.name.drain(..len);
            }
        }
        Ok(())
    }

    fn strip_end(&mut self, substr: &str) -> PatchResult {
        let len = substr.len();
        let glob = globset::Glob::new(&("*".to_string() + substr))?.compile_matcher();
        for ftag in self.fields_mut() {
            if glob.is_match(&ftag.name) {
                let nlen = ftag.name.len();
                ftag.name.truncate(nlen - len);
            }
        }
        Ok(())
    }

    fn modify_field(&mut self, fspec: &str, fmod: &Hash) -> PatchResult {
        let ftags = self.iter_fields(fspec).collect::<Vec<_>>();
        let field_builder = make_field(fmod)?;
        let dim = make_dim_element(fmod)?;
        if !ftags.is_empty() {
            for ftag in ftags {
                modify_dim_element(ftag, &dim)?;
                if let Some(value) = fmod
                    .get(&"_write_constraint".to_yaml())
                    .or_else(|| fmod.get(&"writeConstraint".to_yaml()))
                {
                    let wc = match value {
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
                                min: a[0].i64()? as u64,
                                max: a[1].i64()? as u64,
                            }))
                        }
                        _ => return Err(anyhow!("Unknown writeConstraint type {value:?}")),
                    };
                    ftag.write_constraint = wc;
                }
                // For all other tags, just set the value
                ftag.modify_from(field_builder.clone(), VAL_LVL)?;
                if let Some("") = fmod.get_str("access")? {
                    ftag.access = None;
                }
            }
        }
        Ok(())
    }

    fn add_field(&mut self, fname: &str, fadd: &Hash) -> PatchResult {
        if self.get_field(fname).is_some() {
            return Err(anyhow!(
                "register {} already has a field {fname}",
                self.name
            ));
        }
        let fnew = make_field(fadd)?.name(fname.into()).build(VAL_LVL)?;
        let fnew = if let Some(dim) = make_dim_element(fadd)? {
            fnew.array(dim.build(VAL_LVL)?)
        } else {
            fnew.single()
        };
        self.fields.get_or_insert_with(Default::default).push(fnew);
        Ok(())
    }

    fn delete_field(&mut self, fspec: &str) -> PatchResult {
        if let Some(fields) = self.fields.as_mut() {
            fields.retain(|f| !(matchname(&f.name, fspec)));
        }
        Ok(())
    }

    fn clear_field(&mut self, fspec: &str) -> PatchResult {
        for ftag in self.iter_fields(fspec) {
            if ftag.derived_from.is_some() {
                continue;
            }
            ftag.enumerated_values = Vec::new();
            ftag.write_constraint = None;
        }
        Ok(())
    }

    fn merge_fields(&mut self, key: &str, value: Option<&Yaml>) -> PatchResult {
        let (name, names) = match value {
            Some(Yaml::String(value)) => (
                key.to_string(),
                self.iter_fields(value)
                    .map(|f| f.name.to_string())
                    .collect(),
            ),
            Some(Yaml::Array(value)) => {
                let mut names = Vec::new();
                for fspec in value {
                    names.extend(self.iter_fields(fspec.str()?).map(|f| f.name.to_string()));
                }
                (key.to_string(), names)
            }
            Some(_) => return Err(anyhow!("Invalid usage of merge for {}.{key}", self.name)),
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
            return Err(anyhow!(
                "Could not find any fields to merge {}.{key}",
                self.name
            ));
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
            fields.push(
                FieldInfo::builder()
                    .name(name)
                    .description(desc)
                    .bit_range(BitRange::from_offset_width(bitoffset, bitwidth))
                    .build(VAL_LVL)?
                    .single(),
            );
        }
        Ok(())
    }

    fn collect_fields_in_array(&mut self, fspec: &str, fmod: &Hash) -> PatchResult {
        if let Some(fs) = self.fields.as_mut() {
            let mut fields = Vec::new();
            let mut place = usize::MAX;
            let mut i = 0;
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
                return Err(anyhow!("{}: fields {fspec} not found", self.name));
            }
            fields.sort_by_key(|f| f.bit_range.offset);
            let (li, ri) = spec_ind(fspec);
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
                return Err(anyhow!(
                    "{}: registers cannot be collected into {fspec} array",
                    self.name
                ));
            }
            let mut finfo = fields.swap_remove(0);
            if let Some(name) = fmod.get_str("name")? {
                finfo.name = name.into();
            } else {
                finfo.name = format!("{}%s{}", &fspec[..li], &fspec[fspec.len() - ri..]);
            }
            if let Some(desc) = fmod.get_str("description")? {
                if desc != "_original" {
                    finfo.description = Some(desc.into());
                }
            } else if dim_index[0] == "0" {
                if let Some(desc) = finfo.description.as_mut() {
                    *desc = desc.replace('0', "%s");
                }
            }
            let field = finfo.array(
                DimElement::builder()
                    .dim(dim as u32)
                    .dim_increment(dim_increment)
                    .dim_index(Some(dim_index))
                    .build(VAL_LVL)?,
            );
            //field.process(fmod, &self.name, true);
            fs.insert(place, field);
        }
        Ok(())
    }
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash) -> PatchResult {
        let mut it = self.iter_fields(fspec);
        let (new_fields, name) = match (it.next(), it.next()) {
            (None, _) => {
                return Err(anyhow!(
                    "Could not find any fields to split {}.{fspec}",
                    self.name
                ))
            }
            (Some(_), Some(_)) => {
                return Err(anyhow!(
                    "Only one field can be spitted at time {}.{fspec}",
                    self.name
                ))
            }
            (Some(first), None) => {
                let name = if let Some(n) = fsplit.get_str("name")? {
                    n.to_string()
                } else {
                    first.name.clone() + "%s"
                };
                let desc = if let Some(d) = fsplit.get_str("description")? {
                    Some(d.to_string())
                } else {
                    first.description.clone()
                };
                let bitoffset = first.bit_range.offset;
                let mut fields = Vec::with_capacity(first.bit_range.width as _);
                for i in 0..first.bit_range.width {
                    fields.push({
                        let is = i.to_string();
                        FieldInfo::builder()
                            .name(name.replace("%s", &is))
                            .description(desc.clone().map(|d| d.replace("%s", &is)))
                            .bit_range(BitRange::from_offset_width(bitoffset + i, 1))
                            .build(VAL_LVL)?
                            .single()
                    });
                }
                (fields, first.name.to_string())
            }
        };
        if let Some(fields) = self.fields.as_mut() {
            fields.retain(|f| f.name != name);
            fields.extend(new_fields);
        }
        Ok(())
    }

    fn process_field(&mut self, pname: &str, fspec: &str, fmod: &Yaml) -> PatchResult {
        const READ_KEYS: [&str; 5] = ["_read", "_RM", "_RS", "_RC", "_RME"];
        const READ_VALS: [Option<ReadAction>; 5] = [
            None,
            Some(ReadAction::Modify),
            Some(ReadAction::Set),
            Some(ReadAction::Clear),
            Some(ReadAction::ModifyExternal),
        ];
        const WRITE_KEYS: [&str; 10] = [
            "_write", "_WM", "_WS", "_WC", "_W1S", "_W0C", "_W1C", "_W0S", "_W1T", "_W0T",
        ];
        const WRITE_VALS: [Option<ModifiedWriteValues>; 10] = [
            None,
            Some(ModifiedWriteValues::Modify),
            Some(ModifiedWriteValues::Set),
            Some(ModifiedWriteValues::Clear),
            Some(ModifiedWriteValues::OneToSet),
            Some(ModifiedWriteValues::ZeroToClear),
            Some(ModifiedWriteValues::OneToClear),
            Some(ModifiedWriteValues::ZeroToSet),
            Some(ModifiedWriteValues::OneToToggle),
            Some(ModifiedWriteValues::ZeroToToggle),
        ];
        match fmod {
            Yaml::Hash(fmod) => {
                let is_read = READ_KEYS
                    .iter()
                    .any(|key| fmod.contains_key(&key.to_yaml()));
                let is_write = WRITE_KEYS
                    .iter()
                    .any(|key| fmod.contains_key(&key.to_yaml()));
                if !is_read && !is_write {
                    self.process_field_enum(pname, fspec, fmod, None)
                        .with_context(|| "Adding read-write enumeratedValues")?;
                } else {
                    if is_read {
                        for (key, action) in READ_KEYS.into_iter().zip(READ_VALS.into_iter()) {
                            if let Some(fmod) = fmod.get_hash(key)? {
                                if !fmod.is_empty() {
                                    self.process_field_enum(pname, fspec, fmod, Some(Usage::Read))
                                        .with_context(|| "Adding read-only enumeratedValues")?;
                                }
                                if let Some(action) = action {
                                    self.set_field_read_action(fspec, action);
                                }
                                break;
                            }
                        }
                    }
                    if is_write {
                        for (key, mwv) in WRITE_KEYS.into_iter().zip(WRITE_VALS.into_iter()) {
                            if let Some(fmod) = fmod.get_hash(key)? {
                                if !fmod.is_empty() {
                                    self.process_field_enum(pname, fspec, fmod, Some(Usage::Write))
                                        .with_context(|| "Adding write-only enumeratedValues")?;
                                }
                                if let Some(mwv) = mwv {
                                    self.set_field_modified_write_values(fspec, mwv);
                                }
                            }
                        }
                    }
                }
            }
            Yaml::Array(fmod) if fmod.len() == 2 => {
                self.process_field_range(pname, fspec, fmod)
                    .with_context(|| "Adding writeConstraint range")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn set_field_read_action(&mut self, fspec: &str, action: ReadAction) {
        for ftag in self.iter_fields(fspec) {
            ftag.read_action = Some(action);
        }
    }

    fn set_field_modified_write_values(&mut self, fspec: &str, mwv: ModifiedWriteValues) {
        for ftag in self.iter_fields(fspec) {
            ftag.modified_write_values = if mwv == ModifiedWriteValues::Modify {
                None
            } else {
                Some(mwv)
            };
        }
    }

    fn process_field_enum(
        &mut self,
        pname: &str,
        fspec: &str,
        mut fmod: &Hash,
        usage: Option<Usage>,
    ) -> PatchResult {
        fn set_enum(
            f: &mut FieldInfo,
            mut val: EnumeratedValues,
            usage: Usage,
            replace: bool,
            access: Access,
        ) -> PatchResult {
            let occupied_error = || {
                Err(anyhow!(
                    "field {} already has {usage:?} enumeratedValues",
                    f.name
                ))
            };
            if usage == Usage::ReadWrite {
                if f.enumerated_values.is_empty() || replace {
                    f.enumerated_values = vec![val];
                } else {
                    return occupied_error();
                }
            } else {
                match f.enumerated_values.as_mut_slice() {
                    [] => f.enumerated_values.push(val),
                    [v] if v.usage == Some(usage) || v.usage == Some(Usage::ReadWrite) => {
                        if replace {
                            *v = val;
                        } else {
                            return occupied_error();
                        }
                    }
                    [v] if v.usage.is_none() => match (access, usage) {
                        (Access::ReadWrite | Access::ReadWriteOnce, Usage::Read) => {
                            v.usage = Some(Usage::Write);
                            val.usage = Some(Usage::Read);
                            f.enumerated_values.push(val);
                        }
                        (Access::ReadWrite | Access::ReadWriteOnce, Usage::Write) => {
                            v.usage = Some(Usage::Read);
                            val.usage = Some(Usage::Write);
                            f.enumerated_values.push(val);
                        }
                        _ => {
                            if replace {
                                *v = val;
                            } else {
                                return occupied_error();
                            }
                        }
                    },
                    [_] => f.enumerated_values.push(val),
                    [v1, v2] => {
                        if replace {
                            if v1.usage == Some(usage) {
                                *v1 = val.clone();
                            }
                            if v2.usage == Some(usage) {
                                *v2 = val;
                            }
                        } else {
                            return occupied_error();
                        }
                    }
                    _ => return Err(anyhow!("Incorrect enumeratedValues")),
                }
            }
            Ok(())
        }

        let mut replace_if_exists = false;
        if let Some(h) = fmod.get_hash("_replace_enum")? {
            fmod = h;
            replace_if_exists = true;
        }

        let reg_access = self.properties.access;
        if let Some(d) = fmod.get_str("_derivedFrom")? {
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
                (Some(e), None) => e.usage().ok_or_else(|| {
                    anyhow!("{pname}: multilevel derive for {d} is not supported")
                })?,
                (None, _) => return Err(anyhow!("{pname}: enumeratedValues {d} can't be found")),
                (Some(_), Some(_)) => {
                    return Err(anyhow!(
                        "{pname}: enumeratedValues {d} was found multiple times"
                    ));
                }
            };
            let evs = make_derived_enumerated_values(d)?;
            for ftag in self.iter_fields(fspec) {
                let access = ftag.access.or(reg_access).unwrap_or_default();
                let checked_usage = check_usage(access, usage)
                    .with_context(|| format!("In field {}", ftag.name))?;
                if checked_usage != orig_usage {
                    return Err(anyhow!(
                        "enumeratedValues with different usage was found: {usage:?} != {orig_usage:?}"
                    ));
                }
                if ftag.name == d {
                    return Err(anyhow!("EnumeratedValues can't be derived from itself"));
                }
                set_enum(ftag, evs.clone(), orig_usage, true, access)?;
            }
        } else {
            let offsets = self
                .iter_fields(fspec)
                .map(|f| (f.bit_range.offset, f.name.to_string()))
                .collect::<Vec<_>>();
            if offsets.is_empty() {
                return Err(anyhow!("Could not find {pname}:{}.{fspec}", self.name));
            }
            let (min_offset, fname) = offsets.iter().min_by_key(|on| on.0).unwrap();
            let name = make_ev_name(&fname.replace("%s", ""), usage)?;
            for ftag in self.iter_fields(fspec) {
                let access = ftag.access.or(reg_access).unwrap_or_default();
                let checked_usage = check_usage(access, usage)
                    .with_context(|| format!("In field {}", ftag.name))?;
                if ftag.bit_range.offset == *min_offset {
                    let evs = make_ev_array(fmod)?
                        .name(Some(name.clone()))
                        // TODO: uncomment when python version reaches same functionality
                        //.usage(make_usage(access, checked_usage))
                        .usage(Some(checked_usage))
                        .build(VAL_LVL)?;
                    set_enum(ftag, evs, checked_usage, replace_if_exists, access)?;
                } else {
                    set_enum(
                        ftag,
                        make_derived_enumerated_values(&format!("{fname}.{name}"))?,
                        checked_usage,
                        true,
                        access,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn process_field_range(&mut self, pname: &str, fspec: &str, fmod: &[Yaml]) -> PatchResult {
        let mut set_any = false;
        for ftag in self.iter_fields(fspec) {
            ftag.write_constraint = Some(WriteConstraint::Range(WriteConstraintRange {
                min: fmod[0].i64()? as u64,
                max: fmod[1].i64()? as u64,
            }));
            ftag.validate(ValidateLevel::Weak)?;
            set_any = true;
        }
        if !set_any {
            return Err(anyhow!("Could not find {pname}:{}.{fspec}", self.name));
        }
        Ok(())
    }
}

fn check_usage(access: Access, usage: Option<Usage>) -> anyhow::Result<Usage> {
    Ok(match (access, usage) {
        (Access::ReadWrite | Access::ReadWriteOnce, usage) => usage.unwrap_or_default(),
        (Access::ReadOnly, None | Some(Usage::Read)) => Usage::Read,
        (Access::WriteOnly | Access::WriteOnce, None | Some(Usage::Write)) => Usage::Write,
        (_, _) => {
            return Err(anyhow!(
                "EnumeratedValues usage {usage:?} is incompatible with access {access:?}"
            ));
        }
    })
}

#[allow(unused)]
fn make_usage(access: Access, usage: Usage) -> Option<Usage> {
    match (access, usage) {
        (Access::ReadWrite | Access::ReadWriteOnce, Usage::ReadWrite)
        | (Access::ReadOnly, Usage::Read)
        | (Access::WriteOnly | Access::WriteOnce, Usage::Write) => None,
        _ => Some(usage),
    }
}
