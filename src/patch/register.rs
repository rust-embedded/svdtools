use std::collections::HashSet;

use anyhow::{anyhow, Context};
use itertools::Itertools;
use svd_parser::expand::{BlockPath, RegisterPath};
use svd_parser::svd::{
    Access, BitRange, DimElement, EnumeratedValues, Field, FieldInfo, ModifiedWriteValues,
    ReadAction, Register, Usage, WriteConstraint, WriteConstraintRange,
};
use yaml_rust::{yaml::Hash, Yaml};

use crate::patch::EnumAutoDerive;

use super::iterators::{MatchIter, Matched};
use super::yaml_ext::{AsType, GetVal, ToYaml};
use super::{
    check_offsets, common_description, make_dim_element, matchname, modify_dim_element, spec_ind,
    Config, PatchResult, Spec, VAL_LVL,
};
use super::{make_derived_enumerated_values, make_ev_array, make_ev_name, make_field};

pub type FieldMatchIterMut<'a, 'b> = MatchIter<'b, std::slice::IterMut<'a, Field>>;

/// Collecting methods for processing register contents
pub trait RegisterExt {
    const KEYWORDS: &'static [&'static str] = &[
        "_include",
        "_path",
        "_delete",
        "_derive",
        "_strip",
        "_strip_end",
        "_prefix",
        "_suffix",
        "_clear",
        "_modify",
        "_add",
        "_merge",
        "_split",
        "_array",
    ];

    /// Iterates over all fields that match fspec and live inside rtag
    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b>;

    /// Returns string of present fields
    fn present_fields(&self) -> String;

    /// Work through a register, handling all fields
    fn process(&mut self, rmod: &Hash, bpath: &BlockPath, config: &Config) -> PatchResult;

    /// Add fname given by fadd to rtag
    fn add_field(&mut self, fname: &str, fadd: &Hash, rpath: &RegisterPath) -> PatchResult;

    /// Delete fields matched by fspec inside rtag
    fn delete_field(&mut self, fspec: &str, rpath: &RegisterPath) -> PatchResult;

    /// Clear field from rname and mark it as derivedFrom rderive.
    fn derive_field(&mut self, fname: &str, fderive: &Yaml, rpath: &RegisterPath) -> PatchResult;

    /// Clear contents of fields matched by fspec inside rtag
    fn clear_field(&mut self, fspec: &str) -> PatchResult;

    /// Work through a field, handling either an enum or a range
    fn process_field(
        &mut self,
        fspec: &str,
        fmod: &Yaml,
        rpath: &RegisterPath,
        config: &Config,
    ) -> PatchResult;

    /// Add an enumeratedValues given by field to all fspec in rtag
    fn process_field_enum(
        &mut self,
        fspec: &str,
        fmod: &Hash,
        rpath: &RegisterPath,
        usage: Option<Usage>,
        config: &Config,
    ) -> PatchResult;

    /// Set readAction for field
    fn set_field_read_action(&mut self, fspec: &str, action: ReadAction);

    /// Set modifiedWriteValues for field
    fn set_field_modified_write_values(&mut self, fspec: &str, mwv: ModifiedWriteValues);

    /// Add a writeConstraint range given by field to all fspec in rtag
    fn process_field_range(
        &mut self,
        fspec: &str,
        fmod: &[Yaml],
        rpath: &RegisterPath,
    ) -> PatchResult;

    /// Delete substring from the beginning bitfield names inside rtag
    fn strip_start(&mut self, substr: &str) -> PatchResult;

    /// Delete substring from the ending bitfield names inside rtag
    fn strip_end(&mut self, substr: &str) -> PatchResult;

    /// Add prefix at the beginning of bitfield names inside rtag
    fn add_prefix(&mut self, prefix: &str) -> PatchResult;

    /// Add suffix at the ending of bitfield names inside rtag
    fn add_suffix(&mut self, suffix: &str) -> PatchResult;

    /// Modify fspec inside rtag according to fmod
    fn modify_field(&mut self, fspec: &str, fmod: &Hash, rpath: &RegisterPath) -> PatchResult;

    /// Merge all fspec in rtag.
    /// Support list of field to auto-merge, and dict with fspec or list of fspec
    fn merge_fields(
        &mut self,
        key: &str,
        value: Option<&Yaml>,
        rpath: &RegisterPath,
    ) -> PatchResult;

    /// Split all fspec in rtag.
    /// Name and description can be customized with %s as a placeholder to the iterator value
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash, rpath: &RegisterPath) -> PatchResult;

    /// Collect same fields in peripheral into register array
    fn collect_fields_in_array(
        &mut self,
        fspec: &str,
        fmod: &Hash,
        rpath: &RegisterPath,
    ) -> PatchResult;
}

impl RegisterExt for Register {
    fn iter_fields<'a, 'b>(&'a mut self, spec: &'b str) -> FieldMatchIterMut<'a, 'b> {
        self.fields_mut().matched(spec)
    }

    fn present_fields(&self) -> String {
        self.fields().map(|f| f.name.as_str()).join(", ")
    }

    fn process(&mut self, rmod: &Hash, bpath: &BlockPath, config: &Config) -> PatchResult {
        if self.derived_from.is_some() {
            return Ok(());
        }

        let rpath = bpath.new_register(&self.name);

        // Handle deletions
        for fspec in rmod.str_vec_iter("_delete")? {
            self.delete_field(fspec, &rpath)
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

        if let Some(prefix) = rmod.get_str("_prefix")? {
            self.add_prefix(prefix)
                .with_context(|| format!("Adding prefix `{prefix}` to field names"))?;
        }
        if let Some(suffix) = rmod.get_str("_suffix")? {
            self.add_suffix(suffix)
                .with_context(|| format!("Adding suffix `{suffix}` to field names"))?;
        }

        // Handle field clearing
        for fspec in rmod.str_vec_iter("_clear")? {
            self.clear_field(fspec)
                .with_context(|| format!("Clearing contents of fields matched to `{fspec}`"))?;
        }

        // Handle modifications
        for (fspec, fmod) in rmod.hash_iter("_modify") {
            let fspec = fspec.str()?;
            self.modify_field(fspec, fmod.hash()?, &rpath)
                .with_context(|| format!("Modifying fields matched to `{fspec}`"))?;
        }
        // Handle additions
        for (fname, fadd) in rmod.hash_iter("_add") {
            let fname = fname.str()?;
            self.add_field(fname, fadd.hash()?, &rpath)
                .with_context(|| format!("Adding field `{fname}`"))?;
        }
        // Handle derives
        for (fspec, fderive) in rmod.hash_iter("_derive") {
            let fspec = fspec.str()?;
            self.derive_field(fspec, fderive, &rpath)
                .with_context(|| format!("Deriving field `{fspec}` from `{fderive:?}`"))?;
        }

        // Handle merges
        match rmod.get_yaml("_merge") {
            Some(Yaml::Hash(h)) => {
                for (fspec, fmerge) in h {
                    let fspec = fspec.str()?;
                    self.merge_fields(fspec, Some(fmerge), &rpath)
                        .with_context(|| format!("Merging fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    let fspec = fspec.str()?;
                    self.merge_fields(fspec, None, &rpath)
                        .with_context(|| format!("Merging fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::String(fspec)) => {
                self.merge_fields(fspec, None, &rpath)
                    .with_context(|| format!("Merging fields matched to `{fspec}`"))?;
            }
            _ => {}
        }

        // Handle splits
        match rmod.get_yaml("_split") {
            Some(Yaml::Hash(h)) => {
                for (fspec, fsplit) in h {
                    let fspec = fspec.str()?;
                    self.split_fields(fspec, fsplit.hash()?, &rpath)
                        .with_context(|| format!("Splitting fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::Array(a)) => {
                for fspec in a {
                    let fspec = fspec.str()?;
                    self.split_fields(fspec, &Hash::new(), &rpath)
                        .with_context(|| format!("Splitting fields matched to `{fspec}`"))?;
                }
            }
            Some(Yaml::String(fspec)) => {
                self.split_fields(fspec, &Hash::new(), &rpath)
                    .with_context(|| format!("Splitting fields matched to `{fspec}`"))?;
            }
            _ => {}
        }

        // Handle fields
        if config.update_fields {
            for (fspec, field) in rmod {
                let fspec = fspec.str()?;
                if Self::KEYWORDS.contains(&fspec) {
                    continue;
                }
                self.process_field(fspec, field, &rpath, config)
                    .with_context(|| format!("Processing field matched to `{fspec}`"))?;
            }
        }

        // Handle field arrays
        for (fspec, fmod) in rmod.hash_iter("_array") {
            let fspec = fspec.str()?;
            self.collect_fields_in_array(fspec, fmod.hash()?, &rpath)
                .with_context(|| format!("Collecting fields matched to `{fspec}` in array"))?;
        }

        Ok(())
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

    /// Add prefix at the beginning of bitfield names inside rtag
    fn add_prefix(&mut self, prefix: &str) -> PatchResult {
        for ftag in self.fields_mut() {
            ftag.name.insert_str(0, prefix);
        }
        Ok(())
    }

    /// Add suffix at the ending of bitfield names inside rtag
    fn add_suffix(&mut self, suffix: &str) -> PatchResult {
        for ftag in self.fields_mut() {
            ftag.name.push_str(suffix);
        }
        Ok(())
    }

    fn modify_field(&mut self, fspec: &str, fmod: &Hash, rpath: &RegisterPath) -> PatchResult {
        let (fspec, ignore) = fspec.spec();
        let ftags = self.iter_fields(fspec).collect::<Vec<_>>();
        let field_builder = make_field(fmod, Some(rpath))?;
        let dim = make_dim_element(fmod)?;
        if ftags.is_empty() && !ignore {
            let present = self.present_fields();
            return Err(anyhow!(
                "Could not find `{rpath}:{fspec}. Present fields: {present}.`"
            ));
        } else {
            for ftag in ftags {
                modify_dim_element(ftag, &dim)?;
                if let Some(value) = fmod
                    .get_yaml("_write_constraint")
                    .or_else(|| fmod.get_yaml("writeConstraint"))
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

    fn add_field(&mut self, fname: &str, fadd: &Hash, rpath: &RegisterPath) -> PatchResult {
        if self.get_field(fname).is_some() {
            return Err(anyhow!("register {rpath} already has a field {fname}"));
        }
        let fnew = make_field(fadd, Some(rpath))?
            .name(fname.into())
            .build(VAL_LVL)?;
        let fnew = if let Some(dim) = make_dim_element(fadd)? {
            fnew.array(dim.build(VAL_LVL)?)
        } else {
            fnew.single()
        };
        let exist_bits = self.bitmask();
        if exist_bits & fnew.bitmask() != 0 {
            log::warn!("field {fname} conflicts with other fields in register {rpath}");
        }
        self.fields.get_or_insert_with(Default::default).push(fnew);
        Ok(())
    }

    fn delete_field(&mut self, fspec: &str, rpath: &RegisterPath) -> PatchResult {
        if let Some(fields) = self.fields.as_mut() {
            let mut done = false;
            fields.retain(|f| {
                let del = matchname(&f.name, fspec);
                done |= del;
                !del
            });
            if !done {
                log::info!(
                    "Trying to delete absent `{}` field from register {}",
                    fspec,
                    rpath
                );
            }
        }
        Ok(())
    }

    fn derive_field(&mut self, fspec: &str, fderive: &Yaml, rpath: &RegisterPath) -> PatchResult {
        fn make_path(dpath: &str, rpath: &RegisterPath) -> String {
            let mut parts = dpath.split(".");
            match (parts.next(), parts.next(), parts.next(), parts.next()) {
                (Some(cname), Some(rname), Some(fname), None) if !rpath.block.path.is_empty() => {
                    let fpath = rpath
                        .block
                        .parent()
                        .unwrap()
                        .new_cluster(cname)
                        .new_register(rname)
                        .new_field(fname);
                    fpath.to_string()
                }
                (Some(reg), Some(field), None, None) => {
                    let fpath = rpath.block.new_register(reg).new_field(field);
                    fpath.to_string()
                }
                _ => dpath.into(),
            }
        }
        let (fspec, ignore) = fspec.spec();
        let (dim, info) = if let Some(dpath) = fderive.as_str() {
            (
                None,
                FieldInfo::builder().derived_from(Some(make_path(dpath, rpath))),
            )
        } else if let Some(hash) = fderive.as_hash() {
            let dpath = hash.get_str("_from")?.ok_or_else(|| {
                anyhow!("derive: source field not given, please add a _from field to {fspec}")
            })?;
            (
                make_dim_element(hash)?,
                make_field(hash, Some(rpath))?.derived_from(Some(make_path(dpath, rpath))),
            )
        } else {
            return Err(anyhow!("derive: incorrect syntax for {fspec}"));
        };
        let ftags = self.iter_fields(fspec).collect::<Vec<_>>();
        if !ftags.is_empty() {
            for ftag in ftags {
                modify_dim_element(ftag, &dim)?;
                ftag.modify_from(info.clone(), VAL_LVL)?;
            }
        } else if !ignore {
            super::check_dimable_name(fspec)?;
            let field = info.name(fspec.into()).build(VAL_LVL)?;
            self.fields.get_or_insert(Vec::new()).push({
                if let Some(dim) = dim {
                    field.array(dim.build(VAL_LVL)?)
                } else {
                    field.single()
                }
            });
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

    fn merge_fields(
        &mut self,
        key: &str,
        value: Option<&Yaml>,
        rpath: &RegisterPath,
    ) -> PatchResult {
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
            Some(_) => return Err(anyhow!("Invalid usage of merge for {rpath}.{key}")),
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
            let present = self.present_fields();
            return Err(anyhow!(
                "Could not find any fields to merge {rpath}:{key}. Present fields: {present}.`"
            ));
        }
        if let Some(fields) = self.fields.as_mut() {
            let mut bitwidth = 0;
            let mut bitoffset = u32::MAX;
            let mut pos = usize::MAX;
            let mut first = true;
            let mut desc = None;
            for (i, f) in fields.iter_mut().enumerate() {
                if names.contains(&f.name) {
                    if first {
                        desc.clone_from(&f.description);
                        first = false;
                    }
                    bitwidth += f.bit_range.width;
                    bitoffset = bitoffset.min(f.bit_range.offset);
                    pos = pos.min(i);
                }
            }
            fields.retain(|f| !names.contains(&f.name));
            fields.insert(
                pos,
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

    fn collect_fields_in_array(
        &mut self,
        fspec: &str,
        fmod: &Hash,
        rpath: &RegisterPath,
    ) -> PatchResult {
        if let Some(fs) = self.fields.as_mut() {
            let mut fields = Vec::new();
            let mut place = usize::MAX;
            let mut i = 0;
            let (fspec, ignore) = fspec.spec();
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
                if ignore {
                    return Ok(());
                }
                let present = self.present_fields();
                return Err(anyhow!(
                    "{rpath}: fields {fspec} not found. Present fields: {present}.`"
                ));
            }
            fields.sort_by_key(|f| f.bit_range.offset);
            let Some((li, ri)) = spec_ind(fspec) else {
                return Err(anyhow!(
                    "`{fspec}` contains no tokens or contains more than one token"
                ));
            };
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
                    "{rpath}: fields cannot be collected into {fspec} array. Different bitOffset increments"
                ));
            }
            fields[0].name = if let Some(name) = fmod.get_str("name")? {
                name.into()
            } else {
                format!("{}%s{}", &fspec[..li], &fspec[fspec.len() - ri..])
            };
            if let Some(desc) = fmod.get_str("description")? {
                if desc != "_original" {
                    fields[0].description = Some(desc.into());
                }
            } else {
                let descs: Vec<_> = fields.iter().map(|r| r.description.as_deref()).collect();
                if let Some(desc) = common_description(&descs, &dim_index) {
                    fields[0].description = desc;
                } else {
                    return Err(anyhow!(
                        "{rpath}: fields cannot be collected into {fspec} array. Please, specify description"
                    ));
                }
            }
            let finfo = fields.swap_remove(0);
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
    fn split_fields(&mut self, fspec: &str, fsplit: &Hash, rpath: &RegisterPath) -> PatchResult {
        let (fspec, ignore) = fspec.spec();
        let mut it = self.iter_fields(fspec);
        let (new_fields, name) = match (it.next(), it.next()) {
            (None, _) => {
                if ignore {
                    return Ok(());
                }
                let present = self.present_fields();
                return Err(anyhow!(
                    "Could not find any fields to split {rpath}:{fspec}. Present fields: {present}.`"
                ));
            }
            (Some(_), Some(_)) => {
                return Err(anyhow!(
                    "Only one field can be splitted at time {rpath}:{fspec}"
                ));
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

    fn process_field(
        &mut self,
        fspec: &str,
        fmod: &Yaml,
        rpath: &RegisterPath,
        config: &Config,
    ) -> PatchResult {
        const READ: phf::Map<&'static str, Option<ReadAction>> = phf::phf_map! {
            "_read" => None,
            "_RM" =>  Some(ReadAction::Modify),
            "_RS" => Some(ReadAction::Set),
            "_RC" => Some(ReadAction::Clear),
            "_RME" => Some(ReadAction::ModifyExternal),
        };
        const WRITE: phf::Map<&'static str, Option<ModifiedWriteValues>> = phf::phf_map! {
            "_write" => None,
            "_WM" => Some(ModifiedWriteValues::Modify),
            "_WS" => Some(ModifiedWriteValues::Set),
            "_WC" => Some(ModifiedWriteValues::Clear),
            "_W1S" => Some(ModifiedWriteValues::OneToSet),
            "_W0C" => Some(ModifiedWriteValues::ZeroToClear),
            "_W1C" => Some(ModifiedWriteValues::OneToClear),
            "_W0S" => Some(ModifiedWriteValues::ZeroToSet),
            "_W1T" => Some(ModifiedWriteValues::OneToToggle),
            "_W0T" => Some(ModifiedWriteValues::ZeroToToggle),
        };

        match fmod {
            Yaml::Hash(fmod) => {
                let is_read = READ.keys().any(|key| fmod.contains_key(&key.to_yaml()));
                let is_write = WRITE.keys().any(|key| fmod.contains_key(&key.to_yaml()));
                if !is_read && !is_write {
                    self.process_field_enum(fspec, fmod, rpath, None, config)
                        .context("Adding read-write enumeratedValues")?;
                } else {
                    if is_read {
                        for (&key, action) in &READ {
                            if let Some(fmod) = fmod.get_hash(key)? {
                                if !fmod.is_empty() {
                                    self.process_field_enum(
                                        fspec,
                                        fmod,
                                        rpath,
                                        Some(Usage::Read),
                                        config,
                                    )
                                    .context("Adding read-only enumeratedValues")?;
                                }
                                if let Some(action) = action {
                                    self.set_field_read_action(fspec, *action);
                                }
                                break;
                            }
                        }
                    }
                    if is_write {
                        for (&key, mwv) in &WRITE {
                            if let Some(fmod) = fmod.get_hash(key)? {
                                if !fmod.is_empty() {
                                    self.process_field_enum(
                                        fspec,
                                        fmod,
                                        rpath,
                                        Some(Usage::Write),
                                        config,
                                    )
                                    .context("Adding write-only enumeratedValues")?;
                                }
                                if let Some(mwv) = mwv {
                                    self.set_field_modified_write_values(fspec, *mwv);
                                }
                            }
                        }
                    }
                }
            }
            Yaml::Array(fmod) if fmod.len() == 2 => {
                self.process_field_range(fspec, fmod, rpath)
                    .context("Adding writeConstraint range")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn set_field_read_action(&mut self, fspec: &str, action: ReadAction) {
        let (fspec, _) = fspec.spec();
        for ftag in self.iter_fields(fspec) {
            ftag.read_action = Some(action);
        }
    }

    fn set_field_modified_write_values(&mut self, fspec: &str, mwv: ModifiedWriteValues) {
        let (fspec, _) = fspec.spec();
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
        fspec: &str,
        mut fmod: &Hash,
        rpath: &RegisterPath,
        usage: Option<Usage>,
        config: &Config,
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
                .fields()
                .flat_map(|f| f.enumerated_values.iter())
                .filter(|e| e.name.as_deref() == Some(d));
            let orig_usage = match (derived_enums.next(), derived_enums.next()) {
                (Some(e), None) => e.usage().ok_or_else(|| {
                    anyhow!("{rpath}: multilevel derive for {d} is not supported")
                })?,
                (None, _) => return Err(anyhow!("{rpath}: enumeratedValues {d} can't be found")),
                (Some(_), Some(_)) => {
                    return Err(anyhow!(
                        "{rpath}: enumeratedValues {d} was found multiple times"
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
            let (fspec, ignore) = fspec.spec();
            let mut offsets: Vec<_> = Vec::new();
            let mut width_vals = HashSet::new();
            for (i, f) in self.fields().enumerate() {
                if matchname(&f.name, fspec) {
                    offsets.push((f.bit_offset(), f.name.to_string(), i));
                    width_vals.insert(f.bit_width());
                }
            }
            if offsets.is_empty() {
                if ignore {
                    return Ok(());
                }
                let present = self.present_fields();
                return Err(anyhow!(
                    "Could not find field {rpath}:{fspec}. Present fields: {present}."
                ));
            } else if width_vals.len() > 1 {
                return Err(anyhow!(
                    "{rpath}:{fspec}. Same enumeratedValues are used for different fields."
                ));
            }
            let (min_offset, fname, min_offset_pos) =
                offsets.iter().min_by_key(|&on| on.0).unwrap();
            let min_pos = offsets.iter().map(|on| on.2).min().unwrap();
            let name = if let Some(name) = fmod.get_str("_name")? {
                name.to_string()
            } else {
                make_ev_name(&fname.replace("%s", ""), usage)?
            };
            for ftag in self.iter_fields(fspec) {
                let access = ftag.access.or(reg_access).unwrap_or_default();
                let checked_usage = check_usage(access, usage)
                    .with_context(|| format!("In field {}", ftag.name))?;
                if config.enum_derive == EnumAutoDerive::None || ftag.bit_offset() == *min_offset {
                    let mut evs = make_ev_array(fmod)?.usage(make_usage(access, checked_usage));
                    if ftag.bit_offset() == *min_offset {
                        evs = evs.name(Some(name.clone()));
                    }
                    let evs = evs.build(VAL_LVL)?;
                    set_enum(ftag, evs, checked_usage, replace_if_exists, access)?;
                } else if config.enum_derive == EnumAutoDerive::Field {
                    ftag.modify_from(
                        FieldInfo::builder().derived_from(Some(fname.into())),
                        VAL_LVL,
                    )?;
                } else {
                    set_enum(
                        ftag,
                        make_derived_enumerated_values(&name)?,
                        checked_usage,
                        true,
                        access,
                    )?;
                }
            }
            // Move field with derived enums before other
            if let Some(fields) = self.fields.as_mut() {
                if *min_offset_pos != min_pos {
                    let f = fields.remove(*min_offset_pos);
                    fields.insert(min_pos, f);
                }
            }
        }
        Ok(())
    }

    fn process_field_range(
        &mut self,
        fspec: &str,
        fmod: &[Yaml],
        rpath: &RegisterPath,
    ) -> PatchResult {
        let mut set_any = false;
        let (fspec, ignore) = fspec.spec();
        for ftag in self.iter_fields(fspec) {
            ftag.write_constraint = Some(WriteConstraint::Range(WriteConstraintRange {
                min: fmod[0].i64()? as u64,
                max: fmod[1].i64()? as u64,
            }));
            set_any = true;
        }
        if !ignore && !set_any {
            let present = self.present_fields();
            return Err(anyhow!(
                "Could not find field {rpath}:{fspec}. Present fields: {present}.`"
            ));
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
